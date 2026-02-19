use std::{str::FromStr, sync::Arc};

use axum::{
    Extension, Json, Router,
    extract::{Path, Query, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{get, patch, post, put},
};

use crate::{api::validation::ValidatedJson, models::user::ListUsersQuery};
use serde_json::json;
use sqlx::{Pool, Postgres};
use tracing::{debug, error, info, warn};
use uuid::Uuid;

use crate::{
    db::{
        self,
        user::{
            create_base_user, create_pseudo_user, delete_pseudo_user, get_base_user_by_id,
            list_base_users, patch_base_user_by_id, pseudo_user_exists, tx_create_pseudo_user,
            update_pseudo_user_activity,
        },
    },
    models::{
        app_state::AppState,
        auth::Claims,
        error::ServerError,
        system_log::{LogAction, LogCeverity},
        user::{Auth0User, EnsureUserQuery, PatchUserRequest, Permission, SubjectId, UserRole},
    },
    service::{popup_manager::ClientPopup, system_log_builder::SystemLogBuilder},
};

pub fn public_auth_routes(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/", post(ensure_pseudo_user))
        .route("/popups", get(get_client_popup))
        .with_state(state)
}

pub fn protected_auth_routes(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/", get(list_all_users))
        .route("/me", get(get_base_user_from_subject))
        .route("/{user_id}", patch(patch_user))
        .route("/activity-stats", get(get_user_activity_stats))
        .route("/popups", put(update_client_popup))
        .with_state(state)
}

async fn get_base_user_from_subject(
    State(state): State<Arc<AppState>>,
    Extension(subject_id): Extension<SubjectId>,
    Extension(claims): Extension<Claims>,
) -> Result<impl IntoResponse, ServerError> {
    let user_id = match subject_id {
        SubjectId::BaseUser(user_id) => user_id,
        SubjectId::Integration(_) | SubjectId::PseudoUser(_) => {
            return Err(ServerError::AccessDenied);
        }
    };

    let Some(user) = get_base_user_by_id(state.get_pool(), user_id).await? else {
        let error_msg = format!(
            "User with id {} was previously fetched but is now missing - possible sync issue",
            user_id
        );
        error!("{}", error_msg);
        state
            .syslog()
            .subject(subject_id)
            .action(LogAction::Read)
            .ceverity(LogCeverity::Critical)
            .function("get_user_from_subject")
            .description(&error_msg)
            .log_async();

        return Err(ServerError::NotFound(error_msg));
    };

    let wrapped = match claims.missing_permission([Permission::ReadAdmin, Permission::WriteAdmin]) {
        Some(_missing) => UserRole::BaseUser(user),
        None => UserRole::Admin(user),
    };

    Ok((StatusCode::OK, Json(wrapped)))
}

async fn ensure_pseudo_user(
    State(state): State<Arc<AppState>>,
    Query(query): Query<EnsureUserQuery>,
) -> Result<impl IntoResponse, ServerError> {
    let pseudo_id = match query.pseudo_id {
        None => create_pseudo_user(state.get_pool()).await?,
        Some(mut pseudo_id) => {
            let exists = pseudo_user_exists(state.get_pool(), pseudo_id).await?;
            if exists {
                return Ok((StatusCode::OK, Json(pseudo_id)));
            }

            pseudo_id = create_pseudo_user(state.get_pool()).await?;
            pseudo_id
        }
    };

    let pool = state.get_pool().clone();
    tokio::spawn(async move {
        if let Err(e) = update_pseudo_user_activity(&pool, pseudo_id).await {
            error!(
                "Failed to update pseudo user activity for {}: {}",
                pseudo_id, e
            );
            _ = state
                .syslog()
                .action(LogAction::Update)
                .ceverity(LogCeverity::Warning)
                .function("ensure_pseudo_user")
                .description("Failed to update pseudo user last activity timestamp")
                .metadata(json!({"pseudo_id": pseudo_id, "error": e.to_string()}))
                .log()
                .await;
        };
    });

    Ok((StatusCode::CREATED, Json(pseudo_id)))
}

async fn patch_user(
    State(state): State<Arc<AppState>>,
    Extension(subject): Extension<SubjectId>,
    Extension(claims): Extension<Claims>,
    Path(user_id): Path<Uuid>,
    ValidatedJson(request): ValidatedJson<PatchUserRequest>,
) -> Result<Response, ServerError> {
    let SubjectId::BaseUser(uid) = subject else {
        return Err(ServerError::AccessDenied);
    };

    if claims
        .missing_permission([Permission::WriteAdmin])
        .is_none()
        && user_id != uid
    {
        patch_base_user_by_id(state.get_pool(), &user_id, request).await?;
        return Ok(StatusCode::NO_CONTENT.into_response());
    }

    if request == PatchUserRequest::default() {
        info!("User tried patching without a payload");
        return Ok(StatusCode::NO_CONTENT.into_response());
    }

    let user = patch_base_user_by_id(state.get_pool(), &uid, request).await?;
    Ok((StatusCode::OK, Json(user)).into_response())
}

pub async fn auth0_trigger_endpoint(
    State(state): State<Arc<AppState>>,
    Extension(subject_id): Extension<SubjectId>,
    Path(pseudo_id): Path<String>,
    Json(auth0_user): Json<Auth0User>,
) -> Result<impl IntoResponse, ServerError> {
    let SubjectId::Integration(_intname) = &subject_id else {
        return Err(ServerError::AccessDenied);
    };

    debug!("Recieved pseudo id from auth0: {}", pseudo_id);
    info!(
        "Auth0 post registration trigger was triggered for {}",
        auth0_user.email.clone().unwrap_or("[no email]".to_string())
    );

    let pseudo_id = Uuid::from_str(&pseudo_id).unwrap();

    ensure_no_zombie_pseudo(state.get_pool(), pseudo_id, subject_id);

    let mut tx = state.get_pool().begin().await?;
    let bid = create_base_user(&mut tx, &auth0_user).await?;
    let pid = tx_create_pseudo_user(&mut tx, bid).await?;

    if bid != pid {
        return Err(ServerError::Internal("Failed to create user pair".into()));
    }

    tx.commit().await?;

    Ok((StatusCode::CREATED, Json(pid)))
}

fn ensure_no_zombie_pseudo(pool: &Pool<Postgres>, pseudo_id: Uuid, subject_id: SubjectId) {
    let pool = pool.clone();
    tokio::spawn(async move {
        let pool = pool.clone();
        let subject_id = subject_id.clone();

        match get_base_user_by_id(&pool, pseudo_id).await {
            Ok(option) if option.is_some() => {
                debug!("Base user exists for pseudo user, skipping cleanup");
                return;
            }
            Err(e) => {
                error!(
                    "Failed to fetch base user {} for pseudo user cleanup: {}",
                    pseudo_id, e
                );
                _ = SystemLogBuilder::new(&pool)
                    .action(LogAction::Read)
                    .ceverity(LogCeverity::Warning)
                    .function("cleanup_subject_pseudo_id")
                    .description("Failed to verify base user existence during pseudo user cleanup")
                    .subject(subject_id.clone())
                    .metadata(json!({"pseudo_user_id": pseudo_id, "error": e.to_string()}))
                    .log()
                    .await;

                return;
            }
            _ => {}
        };

        if let Err(e) = delete_pseudo_user(&pool, pseudo_id).await {
            error!("Failed to delete zombie pseudo user {}: {}", pseudo_id, e);
            _ = SystemLogBuilder::new(&pool)
                .action(LogAction::Delete)
                .ceverity(LogCeverity::Critical)
                .function("cleanup_subject_pseudo_id")
                .description("Failed to delete zombie pseudo user without corresponding base user")
                .subject(subject_id)
                .metadata(json!({"pseudo_user_id": pseudo_id, "error": e.to_string()}))
                .log()
                .await;
        };
    });
}

async fn list_all_users(
    State(state): State<Arc<AppState>>,
    Extension(subject_id): Extension<SubjectId>,
    Extension(claims): Extension<Claims>,
    Query(query): Query<ListUsersQuery>,
) -> Result<impl IntoResponse, ServerError> {
    let SubjectId::BaseUser(_) = subject_id else {
        return Err(ServerError::AccessDenied);
    };

    if let Some(missing) = claims.missing_permission([Permission::ReadAdmin]) {
        return Err(ServerError::Permission(missing));
    }

    let users = list_base_users(state.get_pool(), query).await?;
    Ok((StatusCode::OK, Json(users)))
}

async fn get_user_activity_stats(
    State(state): State<Arc<AppState>>,
    Extension(subject_id): Extension<SubjectId>,
    Extension(claims): Extension<Claims>,
) -> Result<impl IntoResponse, ServerError> {
    let SubjectId::BaseUser(_) = subject_id else {
        warn!("Unauthorized guest user or integration attempted to access admin endpoint");
        return Err(ServerError::AccessDenied);
    };

    if let Some(missing) = claims.missing_permission([Permission::ReadAdmin]) {
        warn!("User without admin permissions attempted to access admin endpoint");
        return Err(ServerError::Permission(missing));
    }

    let stats = db::user::get_user_activity_stats(state.get_pool()).await?;
    Ok((StatusCode::OK, Json(stats)))
}

async fn update_client_popup(
    State(state): State<Arc<AppState>>,
    Extension(subject_id): Extension<SubjectId>,
    Extension(claims): Extension<Claims>,
    Json(payload): Json<ClientPopup>,
) -> Result<impl IntoResponse, ServerError> {
    let SubjectId::BaseUser(_user_id) = subject_id else {
        return Err(ServerError::AccessDenied);
    };

    if let Some(missing) = claims.missing_permission([Permission::WriteAdmin]) {
        return Err(ServerError::Permission(missing));
    }

    let manager = state.get_popup_manager();
    let popup = manager.update(payload).await;
    debug!("Popup updated successfully");

    Ok((StatusCode::OK, Json(popup)))
}

pub async fn get_client_popup(
    State(state): State<Arc<AppState>>,
) -> Result<impl IntoResponse, ServerError> {
    let popup = state.get_popup_manager().read().await;
    Ok((StatusCode::OK, Json(popup)))
}
