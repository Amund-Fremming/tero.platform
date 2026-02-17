use std::sync::Arc;

use axum::{
    Extension, Json, Router,
    extract::{Path, Query, State},
    response::IntoResponse,
    routing::{delete, get, patch, post},
};

use crate::api::validation::ValidatedJson;
use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha8Rng;
use reqwest::StatusCode;
use serde_json::json;
use uuid::Uuid;

use tracing::{debug, error, info, warn};

use crate::{
    api::gs_client::{InteractiveGameResponse, JoinGameResponse},
    config::app_config::CONFIG,
    db::{
        self,
        game_base::{
            create_game_base, delete_saved_game, get_game_page, get_saved_games_page, save_game,
            sync_and_increment_times_played, take_random_game,
        },
        imposter_game::create_imposter_game,
        quiz_game::{create_quiz_game, get_quiz_game_by_id},
        spin_game::{create_spin_game, get_spin_game_by_id},
    },
    models::{
        app_state::AppState,
        auth::Claims,
        error::ServerError,
        game_base::{
            CreateGameRequest, GameBase, GameCacheKey, GameConverter, GamePageQuery, GameType,
            InitiateGameRequest, InteractiveEnvelope, ResponseWrapper, SavedGamesPageQuery,
        },
        imposter_game::ImposterSession,
        quiz_game::QuizSession,
        spin_game::SpinSession,
        system_log::{LogAction, LogCeverity},
        user::{Permission, SubjectId},
    },
};

/// NOTE TO SELF
///     Some games can be created as interactive games for people to interact
///     in the creation of the game. But then the game is a standalone game.
///     An example would be quiz, quiz is interactive on creation by adding
///     questions but standalone when playing.
pub fn game_routes(state: Arc<AppState>) -> Router {
    let generic_routes = Router::new()
        .route("/page", post(get_games))
        .route("/{game_id}", delete(delete_game))
        .route("/free-key/{game_key}", patch(free_game_key))
        .route("/save/{game_id}", post(user_save_game))
        .route("/unsave/{game_id}", delete(user_usaved_game))
        .route("/saved", get(get_saved_games))
        .with_state(state.clone());

    let static_routes = Router::new()
        .route(
            "/{game_type}/initiate/{game_id}",
            get(initiate_standalone_game),
        )
        .route("/persist/{game_type}", post(persist_standalone_game))
        .with_state(state.clone());

    let session_routes = Router::new()
        .route("/{game_type}/create", post(create_interactive_game))
        .route("/persist/{game_type}", post(persist_interactive_game))
        .route(
            "/{game_type}/initiate/{game_id}",
            post(initiate_interactive_game),
        )
        .route(
            "/{game_type}/initiate-random",
            post(create_random_interactive_game),
        )
        .route("/join/{game_id}", post(join_interactive_game))
        .with_state(state.clone());

    Router::new()
        .nest("/general", generic_routes)
        .nest("/static", static_routes)
        .nest("/session", session_routes)
}

async fn delete_game(
    State(state): State<Arc<AppState>>,
    Extension(subject_id): Extension<SubjectId>,
    Extension(claims): Extension<Claims>,
    Path(game_id): Path<Uuid>,
) -> Result<impl IntoResponse, ServerError> {
    if let SubjectId::Integration(_) | SubjectId::PseudoUser(_) = subject_id {
        return Err(ServerError::AccessDenied);
    }

    if let Some(missing) = claims.missing_permission([Permission::WriteAdmin]) {
        return Err(ServerError::Permission(missing));
    }

    let deleted_game = db::game_base::delete_game(state.get_pool(), game_id).await?;
    let cache_pointer = state.get_cache().clone();
    let state_pointer = state.clone();

    tokio::spawn(async move {
        if let Err(e) = cache_pointer
            .invalidate(deleted_game.game_type, &deleted_game.category)
            .await
        {
            warn!(
                "Failed to invalidate cache after deleting game {}: {}",
                game_id, e
            );
            state_pointer
                .syslog()
                .action(LogAction::Delete)
                .ceverity(LogCeverity::Warning)
                .function("delete_game")
                .subject(subject_id)
                .description("Failed to invalidate game cache after deletion")
                .metadata(json!({
                    "error": e.to_string(),
                    "game_type": deleted_game.game_type,
                    "game_id": game_id,
                }))
                .log_async();
        };
    });

    Ok(StatusCode::OK)
}

async fn join_interactive_game(
    State(state): State<Arc<AppState>>,
    Extension(subject_id): Extension<SubjectId>,
    Path(key_word): Path<String>,
) -> Result<impl IntoResponse, ServerError> {
    if let SubjectId::Integration(id) = subject_id {
        warn!("Integration {} attempted to access user-only endpoint", id);
        return Err(ServerError::AccessDenied);
    }

    let words: Vec<&str> = key_word.split(" ").collect();
    let tuple = match (words.first(), words.get(1)) {
        (Some(p), Some(s)) => (p.to_string(), s.to_string()),
        _ => {
            warn!("Key word in invalid format");
            return Err(ServerError::Api(
                StatusCode::NOT_FOUND,
                "Game with game key does not exist".into(),
            ));
        }
    };

    let Some(game_type) = state.get_vault().key_active(&tuple) else {
        return Err(ServerError::Api(
            StatusCode::NOT_FOUND,
            "Game with game key does not exist".into(),
        ));
    };

    let hub_address = format!(
        "{}/hubs/{}",
        CONFIG.server.gs_domain,
        game_type.clone().hub_name()
    );
    let response = JoinGameResponse {
        game_key: key_word,
        hub_address,
        game_type,
    };

    Ok((StatusCode::OK, Json(response)))
}

async fn create_interactive_game(
    State(state): State<Arc<AppState>>,
    Extension(subject_id): Extension<SubjectId>,
    Path(game_type): Path<GameType>,
    ValidatedJson(request): ValidatedJson<CreateGameRequest>,
) -> Result<impl IntoResponse, ServerError> {
    let user_id = match subject_id {
        SubjectId::PseudoUser(id) | SubjectId::BaseUser(id) => id,
        _ => return Err(ServerError::AccessDenied),
    };

    let (value, game_base) = match game_type {
        GameType::Roulette => {
            let game_base = GameBase::from_request(&request, GameType::Roulette);
            let session_json = SpinSession::new_roulette(user_id, game_base.id).to_json()?;
            (session_json, game_base)
        }
        GameType::Duel => {
            let game_base = GameBase::from_request(&request, GameType::Duel);
            let session_json = SpinSession::new_duel(user_id, game_base.id).to_json()?;
            (session_json, game_base)
        }
        GameType::Quiz => {
            let game_base = GameBase::from_request(&request, GameType::Quiz);
            let session_json = QuizSession::new(game_base.id).to_json()?;
            (session_json, game_base)
        }
        GameType::Imposter => {
            let game_base = GameBase::from_request(&request, GameType::Imposter);
            let session_json = ImposterSession::new(user_id, game_base.id).to_json()?;
            (session_json, game_base)
        }
    };

    let client = state.get_client();
    let gs_client = state.get_gs_client();
    let pool = state.get_pool();

    // Store game base
    create_game_base(pool, &game_base).await?;
    info!("Persisted interactive game base");

    // Invalidate cache for this game type and category
    let cache = state.get_cache().clone();
    let category = game_base.category.clone();
    let game_type_clone = game_type;
    let state_pointer = state.clone();

    tokio::spawn(async move {
        if let Err(e) = cache.invalidate(game_type_clone, &category).await {
            warn!(
                "Failed to invalidate cache after creating game {}: {}",
                game_base.id, e
            );
            state_pointer
                .syslog()
                .action(LogAction::Create)
                .ceverity(LogCeverity::Warning)
                .function("create_interactive_game")
                .subject(subject_id)
                .description("Failed to invalidate game cache after creation")
                .metadata(json!({
                    "error": e.to_string(),
                    "game_type": game_type_clone,
                }))
                .log_async();
        }
    });

    let key = state.get_vault().create_key(pool, game_type)?;
    let payload = InitiateGameRequest {
        key: key.clone(),
        value,
    };
    debug!("Created key: {}", key);

    gs_client
        .initiate_game_session(client, &game_type, &payload)
        .await?;

    let hub_address = format!("{}/hubs/{}", CONFIG.server.gs_domain, game_type.hub_name());
    let response = InteractiveGameResponse { key, hub_address };

    Ok((StatusCode::CREATED, Json(response)))
}

async fn initiate_standalone_game(
    State(state): State<Arc<AppState>>,
    Extension(subject_id): Extension<SubjectId>,
    Path((game_type, game_id)): Path<(GameType, Uuid)>,
) -> Result<impl IntoResponse, ServerError> {
    let wrapper = match game_type {
        GameType::Quiz => {
            let game = get_quiz_game_by_id(state.get_pool(), &game_id).await?;
            let session = QuizSession::from_game(game);
            ResponseWrapper::Quiz(session)
        }
        _ => {
            return Err(ServerError::Api(
                StatusCode::BAD_REQUEST,
                "This game does not have static support".into(),
            ));
        }
    };

    tokio::task::spawn(async move {
        if let Err(e) = sync_and_increment_times_played(state.get_pool(), game_id).await {
            tracing::error!(
                "Failed to sync and increment times played for {} {}: {}",
                game_type.as_str(),
                game_id,
                e
            );
            state
                .syslog()
                .action(LogAction::Update)
                .ceverity(LogCeverity::Warning)
                .function("initiate_standalone_game")
                .subject(subject_id)
                .description("Failed to sync and increment game play counter for standalone game")
                .metadata(json!({
                    "error": e.to_string(),
                    "game_id": game_id,
                    "game_type": game_type,
                }))
                .log_async();
        }
    });

    Ok((StatusCode::OK, Json(wrapper)))
}

async fn initiate_interactive_game(
    State(state): State<Arc<AppState>>,
    Extension(subject_id): Extension<SubjectId>,
    Path((game_type, game_id)): Path<(GameType, Uuid)>,
) -> Result<impl IntoResponse, ServerError> {
    let user_id = match subject_id {
        SubjectId::PseudoUser(id) | SubjectId::BaseUser(id) => id,
        _ => return Err(ServerError::AccessDenied),
    };

    let client = state.get_client();
    let gs_client = state.get_gs_client();
    let vault = state.get_vault();
    let pool = state.get_pool();

    let value = match game_type {
        GameType::Roulette => {
            let game = get_spin_game_by_id(pool, game_id).await?;
            let session = SpinSession::from_duel(user_id, game);
            session.to_json()?
        }
        GameType::Duel => {
            let game = get_spin_game_by_id(pool, game_id).await?;
            let session = SpinSession::from_roulette(user_id, game);
            session.to_json()?
        }
        _ => {
            return Err(ServerError::Api(
                StatusCode::BAD_REQUEST,
                "This game does not have session support".into(),
            ));
        }
    };

    let key = vault.create_key(pool, game_type)?;
    let payload = InitiateGameRequest {
        key: key.clone(),
        value,
    };

    gs_client
        .initiate_game_session(client, &game_type, &payload)
        .await?;

    let hub_address = format!("{}/hubs/{}", CONFIG.server.gs_domain, game_type.hub_name());
    let response = InteractiveGameResponse { key, hub_address };

    tokio::task::spawn(async move {
        if let Err(e) = sync_and_increment_times_played(state.get_pool(), game_id).await {
            tracing::error!(
                "Failed to sync and increment times played for {} {}: {}",
                game_type.as_str(),
                game_id,
                e
            );
            state
                .syslog()
                .action(LogAction::Update)
                .ceverity(LogCeverity::Warning)
                .function("initiate_interactive_game")
                .subject(subject_id)
                .description("Failed to sync and increment game play counter for interactive game")
                .metadata(json!({
                    "error": e.to_string(),
                    "game_id": game_id,
                    "game_type": game_type,
                }))
                .log_async();
        }
    });

    Ok((StatusCode::OK, Json(response)))
}

async fn create_random_interactive_game(
    State(state): State<Arc<AppState>>,
    Extension(subject_id): Extension<SubjectId>,
    Path(game_type): Path<GameType>,
) -> Result<impl IntoResponse, ServerError> {
    let user_id = match subject_id {
        SubjectId::PseudoUser(id) | SubjectId::BaseUser(id) => id,
        _ => return Err(ServerError::AccessDenied),
    };

    let mut rng = ChaCha8Rng::from_os_rng();
    let len = rng.random_range(4..=7);
    let response = state
        .get_client()
        .get(format!(
            "https://random-word-api.herokuapp.com/word?length={}",
            len
        ))
        .send()
        .await?;
    let status = response.status();

    let name = if !status.is_success() {
        let msg = response.text().await.unwrap_or("No body".to_string());
        error!("Random name api call failed: {} - {}", status, msg);
        format!("Rand{}", len)
    } else {
        let names: Vec<String> = response.json().await?;
        names[0].clone()
    };

    let request = CreateGameRequest::new(name);
    let game = take_random_game(state.get_pool(), &game_type).await?;

    let (value, game_base) = match game_type {
        GameType::Roulette => {
            let game_base = GameBase::from_request(&request, GameType::Roulette);
            let session_json = SpinSession::from_random_roulette(user_id, game).to_json()?;
            (session_json, game_base)
        }
        GameType::Duel => {
            let game_base = GameBase::from_request(&request, GameType::Duel);
            let session_json = SpinSession::from_random_duel(user_id, game).to_json()?;
            (session_json, game_base)
        }
        GameType::Quiz => {
            let game_base = GameBase::from_request(&request, GameType::Quiz);
            let session_json = QuizSession::from_random(game).to_json()?;
            (session_json, game_base)
        }
        GameType::Imposter => {
            let game_base = GameBase::from_request(&request, GameType::Imposter);
            let session_json = ImposterSession::from_random(user_id, game).to_json()?;
            (session_json, game_base)
        }
    };

    let client = state.get_client();
    let gs_client = state.get_gs_client();
    let pool = state.get_pool();

    // Store game base
    create_game_base(pool, &game_base).await?;
    info!("Persisted interactive game base from random game");

    // Invalidate cache for this game type and category
    let cache = state.get_cache().clone();
    let category = game_base.category.clone();
    let game_type_clone = game_type;
    let state_pointer = state.clone();

    tokio::spawn(async move {
        if let Err(e) = cache.invalidate(game_type_clone, &category).await {
            warn!(
                "Failed to invalidate cache after creating random game {}: {}",
                game_base.id, e
            );
            state_pointer
                .syslog()
                .action(LogAction::Create)
                .ceverity(LogCeverity::Warning)
                .function("create_random_interactive_game")
                .subject(subject_id)
                .description("Failed to invalidate game cache after random creation")
                .metadata(json!({
                    "error": e.to_string(),
                    "game_type": game_type_clone,
                }))
                .log_async();
        }
    });

    let key = state.get_vault().create_key(pool, game_type)?;
    let payload = InitiateGameRequest {
        key: key.clone(),
        value,
    };
    debug!("Created key: {}", key);

    gs_client
        .initiate_game_session(client, &game_type, &payload)
        .await?;

    let hub_address = format!("{}/hubs/{}", CONFIG.server.gs_domain, game_type.hub_name());
    let response = InteractiveGameResponse { key, hub_address };

    Ok((StatusCode::CREATED, Json(response)))
}

async fn get_games(
    State(state): State<Arc<AppState>>,
    Extension(subject_id): Extension<SubjectId>,
    Json(request): Json<GamePageQuery>,
) -> Result<impl IntoResponse, ServerError> {
    if let SubjectId::Integration(_) = subject_id {
        warn!("Integration attempted to access game listing endpoint");
        return Err(ServerError::AccessDenied);
    }

    let cache = state.get_cache();
    let cache_key = GameCacheKey::from_query(&request);
    let pool = state.get_pool().clone();
    let request_clone = request.clone();

    let page = cache
        .get_or(cache_key, async move {
            get_game_page(&pool, &request_clone).await
        })
        .await?;

    Ok((StatusCode::OK, Json(page)))
}

pub async fn persist_standalone_game(
    State(state): State<Arc<AppState>>,
    Extension(subject_id): Extension<SubjectId>,
    Path(game_type): Path<GameType>,
    Json(request): Json<InteractiveEnvelope>,
) -> Result<impl IntoResponse, ServerError> {
    if let SubjectId::Integration(id) = subject_id {
        warn!("Integration {} attempted to store a static game", id);
        return Err(ServerError::AccessDenied);
    }

    let game_id = match game_type {
        GameType::Quiz => {
            let session: QuizSession = serde_json::from_value(request.payload)?;
            let game_id = session.game_id;
            create_quiz_game(state.get_pool(), &session.into()).await?;
            game_id
        }
        _ => {
            return Err(ServerError::Api(
                StatusCode::BAD_REQUEST,
                "This game does not have static persist support".into(),
            ));
        }
    };

    tokio::task::spawn(async move {
        if let Err(e) = sync_and_increment_times_played(state.get_pool(), game_id).await {
            tracing::error!(
                "Failed to sync and increment times played for {} {}: {}",
                game_type.as_str(),
                game_id,
                e
            );
            state
                .syslog()
                .action(LogAction::Update)
                .ceverity(LogCeverity::Warning)
                .function("persist_standalone_game")
                .subject(subject_id)
                .description(
                    "Failed to sync and increment game play counter after persisting standalone game",
                )
                .metadata(json!({
                    "error": e.to_string(),
                    "game_id": game_id,
                    "game_type": game_type,
                }))
                .log_async();
        }
    });

    info!("Persisted standalone game");
    Ok(StatusCode::CREATED)
}

/// Only called by `tero.session`.
async fn persist_interactive_game(
    State(state): State<Arc<AppState>>,
    Extension(subject_id): Extension<SubjectId>,
    Extension(claims): Extension<Claims>,
    Path(game_type): Path<GameType>,
    Json(request): Json<InteractiveEnvelope>,
) -> Result<impl IntoResponse, ServerError> {
    let SubjectId::Integration(_) = subject_id else {
        warn!("Non-integration user attempted to persist game session");
        return Err(ServerError::AccessDenied);
    };

    if let Some(missing) = claims.missing_permission([Permission::WriteGame]) {
        return Err(ServerError::Permission(missing));
    }

    let pool = state.get_pool();

    let game_id = match game_type {
        GameType::Roulette | GameType::Duel => {
            let session: SpinSession = serde_json::from_value(request.payload)?;
            let game_id = session.game_id;
            create_spin_game(pool, &session.into()).await?;
            game_id
        }
        GameType::Quiz => {
            let session: QuizSession = serde_json::from_value(request.payload)?;
            let game_id = session.game_id;
            create_quiz_game(pool, &session.into()).await?;
            game_id
        }
        GameType::Imposter => {
            let session: ImposterSession = serde_json::from_value(request.payload)?;
            let game_id = session.game_id;
            create_imposter_game(pool, &session.into()).await?;
            game_id
        }
    };

    tokio::task::spawn(async move {
        if let Err(e) = sync_and_increment_times_played(state.get_pool(), game_id).await {
            error!(
                "Failed to sync and increment times played for {} {}: {}",
                game_type.as_str(),
                game_id,
                e
            );
            state
                .syslog()
                .action(LogAction::Update)
                .ceverity(LogCeverity::Warning)
                .function("persist_interactive_game")
                .subject(subject_id.clone())
                .description("Sync and increment times plated failed")
                .metadata(json!({
                    "error": e.to_string(),
                    "game_id": game_id,
                    "game_type": game_type,
                }))
                .log_async();
        }
    });

    info!("Persisted interactive specialized game");
    Ok(StatusCode::CREATED)
}

async fn free_game_key(
    State(state): State<Arc<AppState>>,
    Extension(subject_id): Extension<SubjectId>,
    Extension(claims): Extension<Claims>,
    Path(game_key): Path<String>,
) -> Result<impl IntoResponse, ServerError> {
    info!("free_game_key endpoint called with key: '{}'", game_key);
    info!("Subject: {:?}", subject_id);

    let SubjectId::Integration(_) = subject_id else {
        warn!("Non-integration user attempted to free game keys");
        return Err(ServerError::AccessDenied);
    };

    if let Some(missing) = claims.missing_permission([Permission::WriteGame]) {
        warn!("Missing permission: {:?}", missing);
        return Err(ServerError::Permission(missing));
    }

    let words: Vec<&str> = game_key.split(" ").collect();
    let tuple = match (words.first(), words.get(1)) {
        (Some(prefix), Some(suffix)) => (prefix.to_string(), suffix.to_string()),
        _ => {
            return Err(ServerError::Api(
                StatusCode::BAD_REQUEST,
                "Key word in invalid format".into(),
            ));
        }
    };

    info!("Game key released: {}", game_key);
    state.get_vault().remove_key(tuple);
    Ok(StatusCode::OK)
}

async fn user_save_game(
    State(state): State<Arc<AppState>>,
    Extension(subject_id): Extension<SubjectId>,
    Path(game_id): Path<Uuid>,
) -> Result<impl IntoResponse, ServerError> {
    let SubjectId::BaseUser(user_id) = subject_id else {
        warn!("Unregistered user or integration tried saving a game");
        return Err(ServerError::AccessDenied);
    };

    save_game(state.get_pool(), user_id, game_id).await?;
    Ok(StatusCode::CREATED)
}

async fn user_usaved_game(
    State(state): State<Arc<AppState>>,
    Extension(subject_id): Extension<SubjectId>,
    Path(game_id): Path<Uuid>,
) -> Result<impl IntoResponse, ServerError> {
    let SubjectId::BaseUser(user_id) = subject_id else {
        warn!("Unregistered user or integration tried unsaving a game");
        return Err(ServerError::AccessDenied);
    };

    delete_saved_game(state.get_pool(), user_id, game_id).await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn get_saved_games(
    State(state): State<Arc<AppState>>,
    Extension(subject_id): Extension<SubjectId>,
    Query(query): Query<SavedGamesPageQuery>,
) -> Result<impl IntoResponse, ServerError> {
    let SubjectId::BaseUser(user_id) = subject_id else {
        warn!("Unregistered user or integration tried fetching saved games");
        return Err(ServerError::AccessDenied);
    };

    let page = get_saved_games_page(state.get_pool(), user_id, query).await?;
    Ok((StatusCode::OK, Json(page)))
}
