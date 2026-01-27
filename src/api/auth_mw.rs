use std::sync::Arc;

use axum::{
    body::Body,
    extract::{Request, State},
    http::{StatusCode, header::AUTHORIZATION},
    middleware::Next,
    response::Response,
};
use jsonwebtoken::{Algorithm, DecodingKey, TokenData, Validation, decode, decode_header};
use serde_json::json;
use sqlx::{Pool, Postgres};

use crate::{
    config::config::CONFIG,
    db::user::{ensure_pseudo_user, get_base_user_by_auth0_id},
    models::{
        app_state::AppState,
        auth::{Claims, Jwks},
        error::ServerError,
        integration::{INTEGRATION_NAMES, IntegrationName},
        system_log::{LogAction, LogCeverity},
        user::SubjectId,
    },
    service::util::{extract_header, to_uuid},
};

static GUEST_AUTHORIZATION: &str = "X-Guest-Authentication";

pub async fn auth_mw(
    State(state): State<Arc<AppState>>,
    mut req: Request<Body>,
    next: Next,
) -> Result<Response, ServerError> {
    let pseudo_header = extract_header(GUEST_AUTHORIZATION, req.headers());
    let token_header = extract_header(AUTHORIZATION.as_str(), req.headers());

    match (pseudo_header, token_header) {
        (Some(pseudo_header), ..) => {
            handle_pseudo_user(state.get_pool(), &mut req, &pseudo_header).await?;
        }
        (None, Some(token_header)) => {
            handle_token_header(state.clone(), &mut req, &token_header).await?;
        }
        _ => {
            tracing::error!("Unauthorized request - no valid authentication header provided");
            return Err(ServerError::AccessDenied);
        }
    };

    Ok(next.run(req).await)
}

async fn handle_pseudo_user(
    pool: &Pool<Postgres>,
    request: &mut Request<Body>,
    pseudo_header: &str,
) -> Result<(), ServerError> {
    let pseudo_id = to_uuid(pseudo_header)?;

    let pool_clone = pool.clone();
    tokio::task::spawn(async move { ensure_pseudo_user(&pool_clone, pseudo_id).await });
    let subject = SubjectId::PseudoUser(pseudo_id);

    request.extensions_mut().insert(subject);
    request.extensions_mut().insert(Claims::empty());

    Ok(())
}

async fn handle_token_header(
    state: Arc<AppState>,
    request: &mut Request<Body>,
    token_header: &str,
) -> Result<(), ServerError> {
    let Some(token) = token_header.strip_prefix("Bearer ") else {
        return Err(ServerError::Api(
            StatusCode::UNAUTHORIZED,
            "Missing auth token".into(),
        ));
    };

    let token_data = verify_jwt(token, state.get_jwks()).await?;
    let claims: Claims = serde_json::from_value(token_data.claims)?;

    let subject = match claims.is_machine() {
        true => {
            let Some(int_name) =
                IntegrationName::from_subject(&claims.sub, &INTEGRATION_NAMES).await
            else {
                tracing::error!(
                    "Unknown integration subject attempted authentication: {}",
                    claims.sub
                );
                return Err(ServerError::AccessDenied);
            };

            SubjectId::Integration(int_name)
        }
        false => {
            let Some(base_user) =
                get_base_user_by_auth0_id(state.get_pool(), claims.auth0_id()).await?
            else {
                tracing::error!(
                    "Failed to find base user for auth0_id {} during authentication",
                    claims.auth0_id()
                );
                state
                    .syslog()
                    .action(LogAction::Read)
                    .ceverity(LogCeverity::Critical)
                    .function("handle_base_user")
                    .description("Auth0 user authenticated but not found in database - sync issue")
                    .metadata(json!({"auth0_id": claims.auth0_id()}))
                    .log_async();

                return Err(ServerError::Internal(
                    "Authentication sync error - user not found in database".into(),
                ));
            };

            SubjectId::BaseUser(base_user.id)
        }
    };

    request.extensions_mut().insert(claims);
    request.extensions_mut().insert(subject);

    Ok(())
}

// Warning: 65% AI generated code
async fn verify_jwt(token: &str, jwks: &Jwks) -> Result<TokenData<serde_json::Value>, ServerError> {
    let header = decode_header(token)
        .map_err(|e| ServerError::JwtVerification(format!("Failed to decode header: {}", e)))?;

    let kid = header
        .kid
        .ok_or_else(|| ServerError::JwtVerification("Missing JWT kid".into()))?;

    let jwk = jwks
        .keys
        .iter()
        .find(|jwk| jwk.kid == kid)
        .ok_or_else(|| ServerError::JwtVerification("JWK is not well known".into()))?;

    let decoding_key = DecodingKey::from_rsa_components(&jwk.n, &jwk.e)
        .map_err(|e| ServerError::JwtVerification(format!("Failed to get decoding key: {}", e)))?;

    let mut validation = Validation::new(Algorithm::RS256);
    validation.set_audience(&[&CONFIG.auth0.audience]);
    validation.set_issuer(&[&CONFIG.auth0.domain]);

    decode::<serde_json::Value>(token, &decoding_key, &validation)
        .map_err(|e| ServerError::JwtVerification(format!("Failed to validate token: {}", e)))
}
