use std::sync::Arc;

use axum::{
    Extension, Json, Router,
    extract::{Query, State},
    response::IntoResponse,
    routing::{get, post},
};
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
    Json(request): Json<CreateGameTipRequest>,
) -> Result<impl IntoResponse, ServerError> {
    // Validate input lengths
    if request.header.len() > 100 {
        return Err(ServerError::Api(
            StatusCode::BAD_REQUEST,
            "Header must be 100 characters or less".into(),
        ));
    }
    
    if request.mobile_phone.len() > 20 {
        return Err(ServerError::Api(
            StatusCode::BAD_REQUEST,
            "Mobile phone must be 20 characters or less".into(),
        ));
    }
    
    if request.description.len() > 500 {
        return Err(ServerError::Api(
            StatusCode::BAD_REQUEST,
            "Description must be 500 characters or less".into(),
        ));
    }
    
    if request.header.trim().is_empty() || request.mobile_phone.trim().is_empty() || request.description.trim().is_empty() {
        return Err(ServerError::Api(
            StatusCode::BAD_REQUEST,
            "Header, mobile phone, and description are required".into(),
        ));
    }
    
    // Anyone can submit a game tip - no authentication required
    let tip_id = db::game_tip::create_game_tip(state.get_pool(), &request).await?;
    
    Ok((StatusCode::CREATED, Json(serde_json::json!({ "id": tip_id }))))
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
