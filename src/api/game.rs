use std::sync::Arc;

use axum::{
    Extension, Json, Router,
    extract::{Path, Query, State},
    response::IntoResponse,
    routing::{delete, get, patch, post},
};

use crate::{
    app_state::AppState,
    db::{
        game_base::{get_random_rounds, increment_times_played},
        imposter_game::get_imposter_game_by_id,
    },
    models::game_base::{CreateStaticGameRequest, GamePagedRequest},
};
use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha8Rng;
use reqwest::StatusCode;
use uuid::Uuid;

use tracing::{debug, error, info, warn};

use crate::{
    api::gs_client::{InteractiveGameResponse, JoinGameResponse},
    db::{
        game_base::{
            create_game_base, delete_saved_game, get_game_page, get_saved_games_page, save_game,
        },
        imposter_game::create_imposter_game,
        quiz_game::{create_quiz_game, get_quiz_game_by_id},
        spin_game::{create_spin_game, get_spin_game_by_id},
    },
    models::{
        auth::Claims,
        error::ServerError,
        game_base::{
            GameBase, GameCacheKey, GameType, InitiateGameRequest, InteractiveGameEnvelope,
            JsonConverter, ResponseWrapper,
        },
        imposter_game::ImposterSession,
        quiz_game::QuizSession,
        spin_game::SpinSession,
        user::{Permission, SubjectId},
    },
};

async fn _get_random_name(client: &reqwest::Client) -> String {
    let mut rng = ChaCha8Rng::from_os_rng();
    let len = rng.random_range(4..=8);
    let result = client
        .get(format!(
            "https://random-word-api.herokuapp.com/word?length={}",
            len
        ))
        .send()
        .await;

    let Ok(response) = result else {
        return String::from("Generic");
    };

    let status = response.status();

    if !status.is_success() {
        return String::from("Generic");
    }

    let Ok(names) = response.json::<Vec<String>>().await else {
        return String::from("Generic");
    };

    dbg!(&names);

    names
        .first()
        .cloned()
        .unwrap_or_else(|| String::from("Generic"))
}

/// NOTE TO SELF
///     Some games can be created as interactive games for people to interact
///     in the creation of the game. But then the game is a standalone game.
///     An example would be quiz, quiz is interactive on creation by adding
///     questions but standalone when playing.
pub fn game_routes(state: Arc<AppState>) -> Router {
    let general_routes = Router::new()
        .route("/page", get(get_games))
        .route("/free-key/{game_key}", patch(free_game_key))
        .route("/save/{game_id}", post(user_save_game))
        .route("/unsave/{game_id}", delete(user_usaved_game))
        .route("/saved", get(get_saved_games))
        .with_state(state.clone());

    let static_routes = Router::new()
        .route("/{game_type}/initiate/{game_id}", get(initiate_static_game))
        .route(
            "/{game_type}/initiate-random",
            get(initiate_random_static_game),
        )
        .route("/persist/{game_type}", post(persist_static_game))
        .with_state(state.clone());

    let session_routes = Router::new()
        .route("/persist/{game_type}", post(persist_interactive_game))
        .route(
            "/{game_type}/initiate/{game_id}",
            post(initiate_interactive_game),
        )
        .route(
            "/{game_type}/initiate-random",
            post(initiate_random_interactive_session),
        )
        .route("/join/{game_id}", post(join_interactive_game))
        .route("/{game_type}/create", post(create_game_session))
        .with_state(state.clone());

    Router::new()
        .nest("/general", general_routes)
        .nest("/static", static_routes)
        .nest("/session", session_routes)
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

    // Check beer tracker first (single-word game IDs)
    let beer_key = key_word.trim().to_lowercase();
    if state.get_beer_cache().contains(&beer_key) {
        let response = JoinGameResponse {
            game_key: beer_key,
            hub_name: "non-hub:beertracker".to_string(),
            game_id: Uuid::nil(),
            game_type: GameType::Roulette,
            is_draft: false,
        };
        return Ok((StatusCode::OK, Json(response)));
    }

    let words: Vec<&str> = key_word.trim().split(" ").collect();
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

    let Some((game_type, is_draft, game_id)) = state.get_vault().key_active(&tuple) else {
        return Err(ServerError::Api(
            StatusCode::NOT_FOUND,
            "Game with game key does not exist".into(),
        ));
    };

    let response = JoinGameResponse {
        game_key: key_word,
        hub_name: game_type.as_str().to_string(),
        game_id,
        game_type,
        is_draft,
    };

    Ok((StatusCode::OK, Json(response)))
}

async fn create_game_session(
    State(state): State<Arc<AppState>>,
    Extension(subject_id): Extension<SubjectId>,
    Path(game_type): Path<GameType>,
) -> Result<impl IntoResponse, ServerError> {
    let user_id = match subject_id {
        SubjectId::PseudoUser(id) | SubjectId::BaseUser(id) => id,
        _ => return Err(ServerError::AccessDenied),
    };

    let game_id = Uuid::new_v4();
    let value = match game_type {
        GameType::Roulette => SpinSession::new_roulette(user_id, game_id).to_json()?,
        GameType::Duel => SpinSession::new_duel(user_id, game_id).to_json()?,
        GameType::Quiz => QuizSession::new(game_id).to_json()?,
        GameType::Imposter => ImposterSession::new(user_id, game_id).to_json()?,
    };

    let key = state
        .get_vault()
        .create_key(state.get_pool(), game_type, true, game_id)?;

    let payload = InitiateGameRequest {
        key: key.clone(),
        value,
    };
    debug!("Created key: {}", key);

    state
        .get_gs_client()
        .initiate_game_session(&game_type, &payload)
        .await?;

    let response = InteractiveGameResponse {
        key,
        game_id,
        hub_name: game_type.hub_name().to_string(),
        is_draft: true,
    };

    Ok((StatusCode::CREATED, Json(response)))
}

async fn initiate_static_game(
    State(state): State<Arc<AppState>>,
    Extension(subject_id): Extension<SubjectId>,
    Path((game_type, game_id)): Path<(GameType, Uuid)>,
) -> Result<impl IntoResponse, ServerError> {
    let user_id = match subject_id {
        SubjectId::BaseUser(id) | SubjectId::PseudoUser(id) => id,
        _ => {
            warn!("Integration tried accessing user endpoint");
            return Err(ServerError::AccessDenied);
        }
    };

    let wrapper = match game_type {
        GameType::Quiz => {
            let game = get_quiz_game_by_id(state.get_pool(), game_id).await?;
            let session = QuizSession::from_game(game);
            ResponseWrapper::Quiz(session)
        }
        GameType::Imposter => {
            let game = get_imposter_game_by_id(state.get_pool(), game_id).await?;
            let session = ImposterSession::from_game(user_id, game);
            ResponseWrapper::Imposter(session)
        }
        _ => {
            return Err(ServerError::Api(
                StatusCode::BAD_REQUEST,
                "This game does not have static support".into(),
            ));
        }
    };

    increment_times_played(state.get_pool(), game_id).await?;

    Ok((StatusCode::OK, Json(wrapper)))
}

async fn initiate_random_static_game(
    State(state): State<Arc<AppState>>,
    Extension(subject_id): Extension<SubjectId>,
    Path(game_type): Path<GameType>,
) -> Result<impl IntoResponse, ServerError> {
    let user_id = match subject_id {
        SubjectId::BaseUser(id) | SubjectId::PseudoUser(id) => id,
        _ => {
            warn!("Integration tried accessing user endpoint");
            return Err(ServerError::AccessDenied);
        }
    };

    let game_id = Uuid::new_v4();
    let wrapper = match game_type {
        GameType::Quiz => {
            let rounds = get_random_rounds(state.get_pool(), game_type, 20).await?;
            ResponseWrapper::Quiz(QuizSession::from_rounds(game_id, rounds))
        }
        GameType::Imposter => {
            let rounds = get_random_rounds(state.get_pool(), game_type, 20).await?;
            ResponseWrapper::Imposter(ImposterSession::from_rounds(user_id, game_id, rounds))
        }
        _ => {
            return Err(ServerError::Api(
                StatusCode::BAD_REQUEST,
                "This game does not have static support".into(),
            ));
        }
    };

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

    let gs_client = state.get_gs_client();
    let vault = state.get_vault();
    let pool = state.get_pool();

    let (value, game_id) = match game_type {
        GameType::Roulette => {
            let game = get_spin_game_by_id(pool, game_id).await?;
            let game_id = game.id;
            let session = SpinSession::from_roulette(user_id, game);
            (session.to_json()?, game_id)
        }
        GameType::Duel => {
            let game = get_spin_game_by_id(pool, game_id).await?;
            let game_id = game.id;
            let session = SpinSession::from_duel(user_id, game);
            (session.to_json()?, game_id)
        }
        _ => {
            return Err(ServerError::Api(
                StatusCode::BAD_REQUEST,
                "This game does not have session support".into(),
            ));
        }
    };

    let key = vault.create_key(pool, game_type, false, game_id)?;
    let payload = InitiateGameRequest {
        key: key.clone(),
        value,
    };

    let (increment_result, game_result) = tokio::join!(
        gs_client.initiate_game_session(&game_type, &payload),
        increment_times_played(state.get_pool(), game_id),
    );

    increment_result?;
    game_result?;

    let response = InteractiveGameResponse {
        key,
        hub_name: game_type.hub_name().to_string(),
        game_id,
        is_draft: false,
    };

    Ok((StatusCode::OK, Json(response)))
}

async fn initiate_random_interactive_session(
    State(state): State<Arc<AppState>>,
    Extension(subject_id): Extension<SubjectId>,
    Path(game_type): Path<GameType>,
) -> Result<impl IntoResponse, ServerError> {
    let user_id = match subject_id {
        SubjectId::PseudoUser(id) | SubjectId::BaseUser(id) => id,
        _ => return Err(ServerError::AccessDenied),
    };

    let game_id = Uuid::new_v4();
    let rounds = get_random_rounds(state.get_pool(), game_type, 20).await?;

    let value = match game_type {
        GameType::Duel => SpinSession::from_duel_rounds(user_id, game_id, rounds).to_json(),
        GameType::Roulette => SpinSession::from_roulette_rounds(user_id, game_id, rounds).to_json(),
        GameType::Imposter => ImposterSession::from_rounds(user_id, game_id, rounds).to_json(),
        _ => {
            error!(
                "Create random interactive game not supported for game type {}",
                game_type.as_str()
            );
            return Err(ServerError::Api(
                StatusCode::BAD_REQUEST,
                "Game type not supported".to_string(),
            ));
        }
    }?;

    let key = state
        .get_vault()
        .create_key(state.get_pool(), game_type, false, game_id)?;

    let payload = InitiateGameRequest {
        key: key.clone(),
        value,
    };
    debug!("Created key: {}", key);

    state
        .get_gs_client()
        .initiate_game_session(&game_type, &payload)
        .await?;

    let response = InteractiveGameResponse {
        key,
        game_id,
        hub_name: game_type.hub_name().to_string(),
        is_draft: false,
    };

    Ok((StatusCode::CREATED, Json(response)))
}

async fn get_games(
    State(state): State<Arc<AppState>>,
    Extension(subject_id): Extension<SubjectId>,
    Query(request): Query<GamePagedRequest>,
) -> Result<impl IntoResponse, ServerError> {
    if let SubjectId::Integration(_) = subject_id {
        warn!("Integration attempted to access game listing endpoint");
        return Err(ServerError::AccessDenied);
    }

    let cache = state.get_cache();
    let cache_key = GameCacheKey::from_request(&request);
    let pool = state.get_pool().clone();
    let request_clone = request.clone();

    let page = cache
        .get_or(cache_key, async move {
            get_game_page(&pool, &request_clone).await
        })
        .await?;

    Ok((StatusCode::OK, Json(page)))
}

/// Called by user
pub async fn persist_static_game(
    State(state): State<Arc<AppState>>,
    Extension(subject_id): Extension<SubjectId>,
    Path(game_type): Path<GameType>,
    Json(payload): Json<CreateStaticGameRequest>,
) -> Result<impl IntoResponse, ServerError> {
    if let SubjectId::Integration(id) = subject_id {
        warn!("Integration {} attempted to store a static game", id);
        return Err(ServerError::AccessDenied);
    }

    let game_base = GameBase::new(
        Uuid::new_v4(),
        payload.name,
        game_type,
        payload.category.clone(),
        payload.rounds.len() as i32,
    );

    let mut tx = state.get_pool().begin().await?;
    create_game_base(tx.as_mut(), &game_base).await?;

    match game_type {
        GameType::Quiz => create_quiz_game(tx.as_mut(), game_base.id, &payload.rounds).await?,
        GameType::Imposter => {
            create_imposter_game(tx.as_mut(), game_base.id, &payload.rounds).await?
        }
        _ => {
            return Err(ServerError::Api(
                StatusCode::BAD_REQUEST,
                format!("Game type {} not supported", game_type.as_str()),
            ));
        }
    };
    tx.commit().await?;

    state.fill_rounds_pool(game_base.id, game_type).await;
    state
        .get_cache()
        .invalidate(game_type, &payload.category)
        .await?;

    info!("Persisted standalone game");
    Ok(StatusCode::CREATED)
}

/// Only called by `tero.session`.
async fn persist_interactive_game(
    State(state): State<Arc<AppState>>,
    Extension(subject_id): Extension<SubjectId>,
    Extension(claims): Extension<Claims>,
    Path(game_type): Path<GameType>,
    Json(payload): Json<InteractiveGameEnvelope>,
) -> Result<impl IntoResponse, ServerError> {
    let SubjectId::Integration(_) = subject_id else {
        warn!("User attempted to persist game session");
        return Err(ServerError::AccessDenied);
    };

    if let Some(missing) = claims.missing_permission([Permission::WriteGame]) {
        return Err(ServerError::Permission(missing));
    }

    let mut tx = state.get_pool().begin().await?;

    let game_id = match game_type {
        GameType::Roulette | GameType::Duel => {
            let session: SpinSession = serde_json::from_value(payload.payload)?;
            let game_base = GameBase::new(
                session.game_id,
                payload.name,
                game_type,
                payload.category.clone(),
                session.rounds.len() as i32,
            );
            create_game_base(tx.as_mut(), &game_base).await?;
            create_spin_game(tx.as_mut(), &session.into()).await?;
            game_base.id
        }
        _ => {
            return Err(ServerError::Api(
                StatusCode::BAD_REQUEST,
                format!("Game type {} not supported", game_type.as_str()),
            ));
        }
    };
    tx.commit().await?;

    state.fill_rounds_pool(game_id, game_type).await;
    state
        .get_cache()
        .invalidate(game_type, &payload.category)
        .await?;

    info!("Persisted interactive game");
    Ok(StatusCode::CREATED)
}

/// Only called by `tero.session`.
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
    Query(query): Query<GamePagedRequest>,
) -> Result<impl IntoResponse, ServerError> {
    let SubjectId::BaseUser(user_id) = subject_id else {
        warn!("Unregistered user or integration tried fetching saved games");
        return Err(ServerError::AccessDenied);
    };

    let page = get_saved_games_page(state.get_pool(), user_id, query).await?;
    Ok((StatusCode::OK, Json(page)))
}
