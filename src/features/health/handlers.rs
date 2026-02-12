use std::sync::Arc;

use axum::{Json, Router, extract::State, response::IntoResponse, routing::get};
use reqwest::StatusCode;
use serde_json::json;

use crate::{
    common::{app_state::AppState, error::ServerError, system_log::{LogAction, LogCeverity}},
    features::health::repository,
};

pub fn health_routes(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/", get(health))
        .route("/detailed", get(health_detailed))
        .with_state(state.clone())
}

async fn health() -> impl IntoResponse {
    "OK".into_response()
}

async fn health_detailed(
    State(state): State<Arc<AppState>>,
) -> Result<impl IntoResponse, ServerError> {
    let platform = true;
    let db_status = repository::health_check(state.get_pool()).await.is_ok();

    let session_status = match state.get_gs_client().health_check(state.get_client()).await {
        Ok(_) => true,
        Err(e) => {
            tracing::error!("Game session health check failed: {}", e);
            state
                .syslog()
                .action(LogAction::Other)
                .ceverity(LogCeverity::Critical)
                .function("health_check")
                .description("Game session service (tero-session) health check failed")
                .metadata(json!({"error": e.to_string()}))
                .log_async();

            false
        }
    };

    let json = json!({
        "platform": platform,
        "database": db_status,
        "session": session_status,
    });

    Ok((StatusCode::OK, Json(json)))
}
