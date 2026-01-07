use std::sync::Arc;

use axum::{
    Extension, Json, Router,
    extract::{Path, Query, State},
    response::IntoResponse,
    routing::{delete, get, patch, post},
};
use reqwest::StatusCode;
use uuid::Uuid;

use tracing::{debug, error, info};

use crate::{
    api::gs_client::{InteractiveGameResponse, JoinGameResponse},
    config::config::CONFIG,
    db::{
        self,
        game_base::{
            create_game_base, delete_saved_game, get_game_page, get_saved_games_page,
            increment_times_played, save_game,
        },
        quiz_game::{create_quiz_game, get_quiz_game_by_id},
        spin_game::{create_spin_game, get_spin_game_by_id},
    },
    models::{
        app_state::AppState,
        auth::Claims,
        error::ServerError,
        game_base::{
            CreateGameRequest, GameBase, GameConverter, GamePageQuery, GameType,
            InitiateGameRequest, InteractiveEnvelope, JsonWrapper, SavedGamesPageQuery,
        },
        quiz_game::QuizSession,
        spin_game::SpinSession,
        user::{Permission, SubjectId},
    },
};

///
/// NOTE TO SELF
///     Some games can be created as interactive games for people to interact
///     in the creation of the game. But then the game is a standalone game.
///     An example would be quiz, quiz is interactive on creation by adding
///     questions but standalone when playing.
pub fn game_routes(state: Arc<AppState>) -> Router {
    let generic_routes = Router::new()
        .route("/page", post(get_games))
        .route("/{game_type}/create", post(create_interactive_game))
        .route("/{game_type}/{game_id}", delete(delete_game))
        .route("/free-key/{game_key}", patch(free_game_key))
        .route("/save/{game_id}", post(user_save_game))
        .route("/unsave/{game_id}", delete(user_usaved_game))
        .route("/saved", get(get_saved_games))
        .with_state(state.clone());

    let standalone_routes = Router::new()
        .route(
            "/{game_type}/initiate/{game_id}",
            get(initiate_standalone_game),
        )
        .route("/persist/{game_type}", post(persist_standalone_game))
        .with_state(state.clone());

    let interactive_routes = Router::new()
        .route(
            "/persist/{game_type}/{game_key}",
            post(persist_interactive_game),
        )
        .route(
            "/{game_type}/initiate/{game_id}",
            post(initiate_interactive_game),
        )
        .route("/join/{game_id}", post(join_interactive_game))
        .with_state(state.clone());

    Router::new()
        .nest("/general", generic_routes)
        .nest("/static", standalone_routes)
        .nest("/session", interactive_routes)
}

async fn delete_game(
    State(state): State<Arc<AppState>>,
    Extension(subject_id): Extension<SubjectId>,
    Extension(claims): Extension<Claims>,
    Path((game_type, game_id)): Path<(GameType, Uuid)>,
) -> Result<impl IntoResponse, ServerError> {
    if let SubjectId::Integration(_) | SubjectId::PseudoUser(_) = subject_id {
        return Err(ServerError::AccessDenied);
    }

    if let Some(missing) = claims.missing_permission([Permission::WriteAdmin]) {
        return Err(ServerError::Permission(missing));
    }

    db::game_base::delete_game(state.get_pool(), &game_type, game_id).await?;
    Ok(StatusCode::OK)
}

async fn join_interactive_game(
    State(state): State<Arc<AppState>>,
    Extension(subject_id): Extension<SubjectId>,
    Path(key_word): Path<String>,
) -> Result<impl IntoResponse, ServerError> {
    if let SubjectId::Integration(id) = subject_id {
        error!("Integration {} tried accessing user endpoint", id);
        return Err(ServerError::AccessDenied);
    }

    let words: Vec<&str> = key_word.split(" ").collect();
    let tuple = match (words.first(), words.get(1)) {
        (Some(p), Some(s)) => (p.to_string(), s.to_string()),
        _ => {
            return Err(ServerError::Api(
                StatusCode::BAD_REQUEST,
                "Key word in invalid format".into(),
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
        game_type.clone().as_str()
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
    Json(request): Json<CreateGameRequest>,
) -> Result<impl IntoResponse, ServerError> {
    let user_id = match subject_id {
        SubjectId::PseudoUser(id) | SubjectId::BaseUser(id) => id,
        _ => return Err(ServerError::AccessDenied),
    };

    let client = state.get_client();
    let gs_client = state.get_gs_client();
    let vault = state.get_vault();
    let pool = state.get_pool();

    let (value, game_base) = match game_type {
        GameType::Roulette => {
            let game_base = GameBase::from_request(&request, GameType::Roulette);
            let session = SpinSession::new_roulette(user_id, game_base.id).to_json_value()?;
            (session, game_base)
        }
        GameType::Duel => {
            let game_base = GameBase::from_request(&request, GameType::Duel);
            let session_json = SpinSession::new_duel(user_id, game_base.id).to_json_value()?;
            (session_json, game_base)
        }
        GameType::Quiz => {
            let game_base = GameBase::from_request(&request, GameType::Quiz);
            let session_json = QuizSession::new().to_json_value()?;
            (session_json, game_base)
        }
    };

    // Store game base
    create_game_base(pool, &game_base).await?;

    let key = vault.create_key(pool, game_type.clone())?;
    let payload = InitiateGameRequest {
        key: key.clone(),
        value,
    };
    info!("Created key: {}", key);

    gs_client
        .initiate_game_session(client, &game_type, &payload)
        .await?;

    let hub_address = format!("{}/hubs/{}", CONFIG.server.gs_domain, game_type.as_str());
    let response = InteractiveGameResponse { key, hub_address };

    debug!("Interactive game was created");
    Ok((StatusCode::CREATED, Json(response)))
}

// TODO - maybe only one detour to database, make increment return full object? or new fn
async fn initiate_standalone_game(
    State(state): State<Arc<AppState>>,
    Extension(_subject_id): Extension<SubjectId>,
    Path((game_type, game_id)): Path<(GameType, Uuid)>,
) -> Result<impl IntoResponse, ServerError> {
    let wrapper = match game_type {
        GameType::Quiz => {
            let game = get_quiz_game_by_id(state.get_pool(), &game_id).await?;
            increment_times_played(state.get_pool(), game.id).await?;
            let session = QuizSession::from_game(game);
            JsonWrapper::QuizWrapper(session)
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

    let client = state.get_client();
    let gs_client = state.get_gs_client();
    let vault = state.get_vault();
    let pool = state.get_pool();

    let value = match game_type {
        GameType::Roulette => {
            let game = get_spin_game_by_id(pool, game_id).await?;
            let session = SpinSession::from_duel(user_id, game);
            session.to_json_value()?
        }
        GameType::Duel => {
            let game = get_spin_game_by_id(pool, game_id).await?;
            let session = SpinSession::from_roulette(user_id, game);
            session.to_json_value()?
        }
        _ => {
            return Err(ServerError::Api(
                StatusCode::BAD_REQUEST,
                "This game does not have session support".into(),
            ));
        }
    };

    let key = vault.create_key(pool, game_type.clone())?;
    let payload = InitiateGameRequest {
        key: key.clone(),
        value,
    };

    gs_client
        .initiate_game_session(client, &game_type, &payload)
        .await?;

    let hub_address = format!("{}/hubs/{}", CONFIG.server.gs_domain, game_type.as_str());

    let response = InteractiveGameResponse { key, hub_address };

    Ok((StatusCode::OK, Json(response)))
}

async fn get_games(
    State(state): State<Arc<AppState>>,
    Extension(subject_id): Extension<SubjectId>,
    Json(request): Json<GamePageQuery>,
) -> Result<impl IntoResponse, ServerError> {
    if let SubjectId::Integration(_) = subject_id {
        return Err(ServerError::AccessDenied);
    }

    // TODO - add back caching: it has a issue not displaying new entries
    /*
    let cache = state.get_cache();

    let page = cache
        .get_or(&request, || get_game_page(pool, &request))
        .await?;
    */

    let page = get_game_page(state.get_pool(), &request).await?;
    Ok((StatusCode::OK, Json(page)))
}

pub async fn persist_standalone_game(
    State(state): State<Arc<AppState>>,
    Extension(subject_id): Extension<SubjectId>,
    Path(game_type): Path<GameType>,
    Json(request): Json<InteractiveEnvelope>,
) -> Result<impl IntoResponse, ServerError> {
    if let SubjectId::Integration(id) = subject_id {
        error!("Integration {} tried to store a static game", id);
        return Err(ServerError::AccessDenied);
    }

    match game_type {
        GameType::Quiz => {
            let session: QuizSession = serde_json::from_value(request.payload)?;
            increment_times_played(state.get_pool(), session.game_id).await?;
            create_quiz_game(state.get_pool(), &session.into()).await?;
        }
        _ => {
            return Err(ServerError::Api(
                StatusCode::BAD_REQUEST,
                "This game does not have static persist support".into(),
            ));
        }
    }

    info!("Persisted standalone game");
    Ok(StatusCode::CREATED)
}

async fn persist_interactive_game(
    State(state): State<Arc<AppState>>,
    Extension(subject_id): Extension<SubjectId>,
    Extension(claims): Extension<Claims>,
    Path((game_type, game_key)): Path<(GameType, String)>,
    Json(request): Json<InteractiveEnvelope>,
) -> Result<impl IntoResponse, ServerError> {
    let SubjectId::Integration(_) = subject_id else {
        error!("User tried to persist game session");
        return Err(ServerError::AccessDenied);
    };

    if let Some(missing) = claims.missing_permission([Permission::WriteGame]) {
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

    state.get_vault().remove_key(tuple);
    let pool = state.get_pool();
    info!("Removed game key: {}", game_key);

    match game_type {
        GameType::Roulette | GameType::Duel => {
            let session: SpinSession = serde_json::from_value(request.payload)?;
            increment_times_played(pool, session.game_id).await?;
            create_spin_game(pool, &session.into()).await?;
        }
        GameType::Quiz => {
            let session: QuizSession = serde_json::from_value(request.payload)?;
            increment_times_played(pool, session.game_id).await?;
            create_quiz_game(pool, &session.into()).await?;
        }
    }

    info!("Persisted interactive game");
    Ok(StatusCode::CREATED)
}

async fn free_game_key(
    State(state): State<Arc<AppState>>,
    Extension(subject_id): Extension<SubjectId>,
    Extension(claims): Extension<Claims>,
    Path(game_key): Path<String>,
) -> Result<impl IntoResponse, ServerError> {
    let SubjectId::Integration(_) = subject_id else {
        error!("User tried to free game keys/word");
        return Err(ServerError::AccessDenied);
    };

    if let Some(missing) = claims.missing_permission([Permission::WriteGame]) {
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

    info!("Game key freed: {}", game_key);
    state.get_vault().remove_key(tuple);
    Ok(StatusCode::OK)
}

async fn user_save_game(
    State(state): State<Arc<AppState>>,
    Extension(subject_id): Extension<SubjectId>,
    Path(game_id): Path<Uuid>,
) -> Result<impl IntoResponse, ServerError> {
    let SubjectId::BaseUser(user_id) = subject_id else {
        error!("Unregistered user or integration tried saving a game");
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
        error!("Unregistered user or integration tried saving a game");
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
        error!("Unregistered user or integration tried saving a game");
        return Err(ServerError::AccessDenied);
    };

    let page = get_saved_games_page(state.get_pool(), user_id, query).await?;
    Ok((StatusCode::OK, Json(page)))
}
