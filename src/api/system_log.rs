use std::sync::Arc;

use tracing::info;

use axum::{
    Extension, Json, Router,
    extract::{Query, State},
    response::IntoResponse,
    routing::{get, post},
};
use reqwest::StatusCode;

use crate::{
    db,
    models::{
        app_state::AppState,
        auth::Claims,
        error::ServerError,
        system_log::{CreateSyslogRequest, SyslogPageQuery},
        user::{Permission, SubjectId},
    },
};

pub fn log_routes(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/", post(create_system_log).get(get_system_log_page))
        .route("/count", get(get_log_category_count))
        .with_state(state)
}

async fn get_system_log_page(
    State(state): State<Arc<AppState>>,
    Extension(subject_id): Extension<SubjectId>,
    Extension(claims): Extension<Claims>,
    Query(query): Query<SyslogPageQuery>,
) -> Result<impl IntoResponse, ServerError> {
    let SubjectId::BaseUser(_) = subject_id else {
        tracing::error!("Unauthorized subject attempted to read system logs");
        return Err(ServerError::AccessDenied);
    };

    if let Some(missing) = claims.missing_permission([Permission::ReadAdmin]) {
        return Err(ServerError::Permission(missing));
    }

    let page = db::system_log::get_system_log_page(state.get_pool(), query).await?;
    Ok((StatusCode::OK, Json(page)))
}

async fn create_system_log(
    State(state): State<Arc<AppState>>,
    Extension(subject_id): Extension<SubjectId>,
    Extension(claims): Extension<Claims>,
    Json(request): Json<CreateSyslogRequest>,
) -> Result<impl IntoResponse, ServerError> {
    match &subject_id {
        SubjectId::PseudoUser(id) | SubjectId::BaseUser(id) => {
            tracing::error!("User {} attempted to write a system log without permission", id);
            return Err(ServerError::AccessDenied);
        }
        SubjectId::Integration(int_name) => {
            if let Some(missing) = claims.missing_permission([Permission::WriteSystemLog]) {
                return Err(ServerError::Permission(missing));
            }

            info!("Integration {} is creating a system log entry", int_name);
        }
    };

    let mut builder = state.syslog().subject(subject_id);

    if let Some(action) = request.action {
        builder = builder.action(action);
    };

    if let Some(ceverity) = request.ceverity {
        builder = builder.ceverity(ceverity);
    }

    if let Some(description) = request.description {
        builder = builder.description(&description);
    }

    if let Some(metadata) = request.metadata {
        builder = builder.metadata(metadata);
    }

    if let Some(function) = request.function {
        builder = builder.function(&function);
    }

    builder.log_async();

    Ok(StatusCode::CREATED)
}

async fn get_log_category_count(
    State(state): State<Arc<AppState>>,
    Extension(subject_id): Extension<SubjectId>,
    Extension(claims): Extension<Claims>,
) -> Result<impl IntoResponse, ServerError> {
    let SubjectId::BaseUser(_) = subject_id else {
        tracing::error!("Unauthorized subject attempted to read log category counts");
        return Err(ServerError::AccessDenied);
    };

    if let Some(missing) = claims.missing_permission([Permission::ReadAdmin]) {
        return Err(ServerError::Permission(missing));
    }

    let counts = db::system_log::get_log_category_count(state.get_pool()).await?;
    Ok((StatusCode::OK, Json(counts)))
}
