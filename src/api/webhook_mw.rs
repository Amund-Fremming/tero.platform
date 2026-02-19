use axum::{body::Body, extract::Request, middleware::Next, response::Response};
use reqwest::StatusCode;
use tracing::{info, warn};

use crate::{
    config::app_config::CONFIG,
    models::{error::ServerError, integration::IntegrationName, user::SubjectId},
    service::util::extract_header,
};

static AUTH0_WEBHOOK_KEY: &str = "Auth0-Webhook-Key";

pub async fn webhook_mw(mut req: Request<Body>, next: Next) -> Result<Response, ServerError> {
    let webhook_header = extract_header(AUTH0_WEBHOOK_KEY, req.headers()).ok_or_else(|| {
        ServerError::Api(StatusCode::UNAUTHORIZED, "Webhook key not present".into())
    })?;

    let valid_key = CONFIG.auth0.webhook_key.to_string();
    if valid_key != webhook_header {
        warn!(
            "Valid key: {} \nReceived webhook key: {} ",
            valid_key, webhook_header
        );
        return Err(ServerError::Api(
            StatusCode::UNAUTHORIZED,
            "Invalid webhook key".into(),
        ));
    }

    let subject = SubjectId::Integration(IntegrationName::Auth0);
    info!("Request by subject: {:?}", subject);
    req.extensions_mut().insert(subject);

    Ok(next.run(req).await)
}
