use std::sync::Arc;

use axum::{
    Extension, Json, Router,
    extract::{Query, State},
    response::IntoResponse,
    routing::{get, post},
};
use axum_valid::Valid;
use reqwest::StatusCode;
use tracing::error;

use crate::{
    db,
    models::{
        app_state::AppState,
        auth::Claims,
        error::ServerError,
        game_tip::{CreateGameTipRequest, GameTipPageQuery},
        user::{Permission, SubjectId},
    },
};

pub fn public_game_tip_routes(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/", post(create_game_tip))
        .with_state(state)
}

pub fn protected_game_tip_routes(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/admin", get(get_game_tips_admin))
        .with_state(state)
}

async fn create_game_tip(
    State(state): State<Arc<AppState>>,
    Valid(Json(request)): Valid<Json<CreateGameTipRequest>>,
) -> Result<impl IntoResponse, ServerError> {
    let tip_id = db::game_tip::create_game_tip(state.get_pool(), &request).await?;

    Ok((
        StatusCode::CREATED,
        Json(serde_json::json!({ "id": tip_id })),
    ))
}

async fn get_game_tips_admin(
    State(state): State<Arc<AppState>>,
    Extension(subject_id): Extension<SubjectId>,
    Extension(claims): Extension<Claims>,
    Query(query): Query<GameTipPageQuery>,
) -> Result<impl IntoResponse, ServerError> {
    // Only admins can fetch game tips
    let SubjectId::BaseUser(_) = subject_id else {
        error!("Unauthorized subject tried reading game tips");
        return Err(ServerError::AccessDenied);
    };

    if let Some(missing) = claims.missing_permission([Permission::ReadAdmin]) {
        return Err(ServerError::Permission(missing));
    }

    let page = db::game_tip::get_game_tips_page(state.get_pool(), query.page_num).await?;
    Ok((StatusCode::OK, Json(page)))
}
