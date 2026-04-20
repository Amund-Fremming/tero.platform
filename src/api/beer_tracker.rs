use std::{sync::Arc, time::Duration};

use axum::{
    Json, Router,
    extract::{Path, State},
    response::IntoResponse,
    routing::{get, post},
};
use moka::future::Cache;
use reqwest::StatusCode;
use tracing::{debug, info};

use crate::{
    api::validation::ValidatedJson,
    app_state::AppState,
    db,
    models::{
        beer_tracker::{
            BeerTrackerGame, CreateBeerTrackerRequest, IncrementBeerRequest,
            JoinBeerTrackerRequest, LeaveBeerTrackerRequest, UserScore,
        },
        error::ServerError,
    },
};

#[derive(Clone)]
pub struct BeerTrackerCache {
    cache: Arc<Cache<String, BeerTrackerGame>>,
}

impl BeerTrackerCache {
    pub fn new() -> Self {
        let cache = Cache::builder()
            .max_capacity(5_000)
            .time_to_idle(Duration::from_secs(300))
            .build();
        Self {
            cache: Arc::new(cache),
        }
    }

    pub async fn get(&self, id: &str) -> Option<BeerTrackerGame> {
        self.cache.get(id).await
    }

    pub async fn set(&self, game: BeerTrackerGame) {
        self.cache.insert(game.id.clone(), game).await;
    }

    pub async fn invalidate(&self, id: &str) {
        self.cache.invalidate(id).await;
    }

    pub fn contains(&self, id: &str) -> bool {
        self.cache.contains_key(id)
    }
}

pub fn beer_tracker_routes(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/", post(create_game))
        .route("/{id}", get(get_game))
        .route("/{id}/join", post(join_game))
        .route("/{id}/increment", post(increment_beer))
        .route("/{id}/leave", post(leave_game))
        .route("/{id}/finish", post(finish_game))
        .with_state(state)
}

async fn fetch_game_state(state: &Arc<AppState>, id: &str) -> Result<BeerTrackerGame, ServerError> {
    if let Some(cached) = state.get_beer_cache().get(id).await {
        return Ok(cached);
    }

    let game_row = db::beer_tracker::get_game(state.get_pool(), id)
        .await?
        .ok_or_else(|| ServerError::NotFound("Game not found".into()))?;

    let member_rows = db::beer_tracker::get_members(state.get_pool(), id).await?;

    let game = BeerTrackerGame {
        id: game_row.id,
        can_size: game_row.can_size,
        goal: game_row.goal,
        members: member_rows
            .into_iter()
            .map(|m| UserScore {
                name: m.name,
                count: m.count,
            })
            .collect(),
    };

    state.get_beer_cache().set(game.clone()).await;
    Ok(game)
}

async fn create_game(
    State(state): State<Arc<AppState>>,
    ValidatedJson(request): ValidatedJson<CreateBeerTrackerRequest>,
) -> Result<impl IntoResponse, ServerError> {
    if request.can_size != 0.33 && request.can_size != 0.5 {
        return Err(ServerError::Api(
            StatusCode::BAD_REQUEST,
            "Can size must be 0.33 or 0.5".into(),
        ));
    }

    let id = request.game_id.trim().to_lowercase();

    if db::beer_tracker::game_exists(state.get_pool(), &id).await? {
        return Err(ServerError::Api(
            StatusCode::CONFLICT,
            "Game ID is already taken".into(),
        ));
    }

    db::beer_tracker::create_game(
        state.get_pool(),
        &id,
        request.can_size,
        request.goal,
        request.name.trim(),
    )
    .await?;

    info!("Beer tracker game created: {}", id);

    let game = fetch_game_state(&state, &id).await?;
    Ok((StatusCode::CREATED, Json(game)))
}

async fn get_game(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<impl IntoResponse, ServerError> {
    let game = fetch_game_state(&state, &id).await?;
    Ok((StatusCode::OK, Json(game)))
}

async fn join_game(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    ValidatedJson(request): ValidatedJson<JoinBeerTrackerRequest>,
) -> Result<impl IntoResponse, ServerError> {
    let name = request.name.trim().to_string();

    if !db::beer_tracker::game_exists(state.get_pool(), &id).await? {
        return Err(ServerError::NotFound("Game not found".into()));
    }

    if db::beer_tracker::member_exists(state.get_pool(), &id, &name).await? {
        return Err(ServerError::Api(
            StatusCode::CONFLICT,
            "Name is already taken in this game".into(),
        ));
    }

    db::beer_tracker::add_member(state.get_pool(), &id, &name).await?;
    state.get_beer_cache().invalidate(&id).await;

    debug!("Player '{}' joined beer tracker game '{}'", name, id);

    let game = fetch_game_state(&state, &id).await?;
    Ok((StatusCode::OK, Json(game)))
}

async fn increment_beer(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    ValidatedJson(request): ValidatedJson<IncrementBeerRequest>,
) -> Result<impl IntoResponse, ServerError> {
    if request.can_size != 0.33 && request.can_size != 0.5 {
        return Err(ServerError::Api(
            StatusCode::BAD_REQUEST,
            "Can size must be 0.33 or 0.5".into(),
        ));
    }

    let name = request.name.trim().to_string();

    if !db::beer_tracker::member_exists(state.get_pool(), &id, &name).await? {
        return Err(ServerError::Api(
            StatusCode::FORBIDDEN,
            "You are not a member of this game".into(),
        ));
    }

    // Fetch game to get the base can_size for proportional scoring
    let game = fetch_game_state(&state, &id).await?;

    // Calculate proportional increment: if game is 0.33 and you add 0.5, that's ~1.52 units
    // We store count as integer (units of the base can size), so we round
    let increment = if (game.can_size - request.can_size).abs() < 0.01 {
        100 // same size = 100 (we use centiunits for precision)
    } else {
        ((request.can_size / game.can_size) * 100.0).round() as i32
    };

    db::beer_tracker::increment_count(state.get_pool(), &id, &name, increment).await?;
    state.get_beer_cache().invalidate(&id).await;

    let game = fetch_game_state(&state, &id).await?;
    Ok((StatusCode::OK, Json(game)))
}

async fn leave_game(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    ValidatedJson(request): ValidatedJson<LeaveBeerTrackerRequest>,
) -> Result<impl IntoResponse, ServerError> {
    let name = request.name.trim().to_string();

    db::beer_tracker::remove_member(state.get_pool(), &id, &name).await?;

    let remaining = db::beer_tracker::member_count(state.get_pool(), &id).await?;
    if remaining == 0 {
        db::beer_tracker::delete_game(state.get_pool(), &id).await?;
        state.get_beer_cache().invalidate(&id).await;
        info!("Beer tracker game '{}' deleted (no members left)", id);
        return Ok((StatusCode::OK, Json(serde_json::json!({"deleted": true}))));
    }

    state.get_beer_cache().invalidate(&id).await;
    let game = fetch_game_state(&state, &id).await?;
    Ok((StatusCode::OK, Json(serde_json::to_value(game).unwrap())))
}

async fn finish_game(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<impl IntoResponse, ServerError> {
    db::beer_tracker::delete_game(state.get_pool(), &id).await?;
    state.get_beer_cache().invalidate(&id).await;
    info!("Beer tracker game '{}' finished", id);
    Ok((StatusCode::OK, Json(serde_json::json!({"deleted": true}))))
}
