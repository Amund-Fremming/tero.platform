use core::fmt;
use std::{collections::HashMap, hash::Hash};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

// ============================================================================
// Traits and Common Types
// ============================================================================

pub trait GameConverter {
    fn to_json(&self) -> Result<serde_json::Value, serde_json::Error>;
}

#[derive(Debug, sqlx::FromRow)]
pub struct DeleteGameResult {
    pub game_type: GameType,
    pub category: GameCategory,
}

#[allow(dead_code)]
#[derive(Serialize)]
#[serde(untagged)]
pub enum ResponseWrapper {
    Quiz(QuizSession),
    Spin(SpinSession),
    Imposter(ImposterSession),
}

// ============================================================================
// Game Base Types
// ============================================================================

#[derive(Debug, Serialize, Deserialize, Clone, sqlx::FromRow)]
pub struct GameBase {
    pub id: Uuid,
    pub name: String,
    pub game_type: GameType,
    pub category: GameCategory,
    pub iterations: i32,
    pub times_played: i32,
    pub last_played: DateTime<Utc>,
    pub synced: bool,
}

impl GameBase {
    pub fn from_request(request: &CreateGameRequest, game_type: GameType) -> Self {
        Self {
            id: Uuid::new_v4(),
            name: request.name.clone(),
            game_type,
            category: request.category.clone(),
            iterations: 0,
            times_played: 0,
            last_played: Utc::now(),
            synced: false,
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Hash, Clone, PartialEq, Eq, sqlx::Type)]
#[sqlx(type_name = "game_category", rename_all = "lowercase")]
pub enum GameCategory {
    Vors,
    Ladies,
    Boys,
    All,
}

impl fmt::Display for GameCategory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GameCategory::Ladies => write!(f, "ladies"),
            GameCategory::Boys => write!(f, "boys"),
            GameCategory::Vors => write!(f, "vors"),
            GameCategory::All => write!(f, "all"),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, sqlx::Type, PartialEq)]
#[sqlx(type_name = "gender", rename_all = "lowercase")]
pub enum Gender {
    #[sqlx(rename = "m")]
    Male,
    #[sqlx(rename = "f")]
    Female,
    #[sqlx(rename = "u")]
    Unknown,
}

#[derive(Debug, Serialize, Deserialize, Hash, Clone, PartialEq, Eq, sqlx::Type, Copy)]
#[sqlx(type_name = "game_type", rename_all = "lowercase")]
pub enum GameType {
    Roulette,
    Duel,
    Quiz,
    Imposter,
}

impl GameType {
    pub fn as_str(&self) -> &'static str {
        match self {
            GameType::Quiz => "quiz",
            GameType::Duel => "duel",
            GameType::Roulette => "roulette",
            GameType::Imposter => "imposter",
        }
    }

    pub fn hub_name(&self) -> &'static str {
        match self {
            GameType::Quiz => "quiz",
            GameType::Duel | GameType::Roulette => "spin",
            GameType::Imposter => "imposter",
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Hash, Eq, PartialEq, Clone)]
pub struct GameCacheKey {
    pub page_num: u16,
    pub game_type: GameType,
    pub category: Option<GameCategory>,
}

impl GameCacheKey {
    pub fn from_query(query: &GamePageQuery) -> Self {
        Self {
            page_num: query.page_num,
            game_type: query.game_type,
            category: query.category.clone(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Hash, Clone)]
pub struct GamePageQuery {
    pub page_num: u16,
    pub game_type: GameType,
    pub category: Option<GameCategory>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SavedGamesPageQuery {
    pub page_num: Option<u8>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct InteractiveEnvelope {
    pub payload: serde_json::Value,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CreateGameRequest {
    pub name: String,
    pub category: GameCategory,
}

impl CreateGameRequest {
    pub fn new(name: String) -> Self {
        Self {
            name,
            category: GameCategory::All,
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct InitiateGameRequest {
    pub key: String,
    pub value: serde_json::Value,
}

#[allow(dead_code)] // TODO!
#[derive(Debug, sqlx::FromRow)]
pub struct RandomGame {
    pub id: i64,
    pub game_id: Uuid,
    pub rounds: Vec<String>,
    pub game_type: GameType,
}

// ============================================================================
// Quiz Game Types
// ============================================================================

impl GameConverter for QuizSession {
    fn to_json(&self) -> Result<serde_json::Value, serde_json::Error> {
        serde_json::to_value(self)
    }
}

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct QuizGame {
    pub id: Uuid,
    pub rounds: Vec<String>,
}

impl From<QuizSession> for QuizGame {
    fn from(value: QuizSession) -> Self {
        Self {
            id: value.game_id,
            rounds: value.rounds,
        }
    }
}

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct QuizSession {
    pub game_id: Uuid,
    pub current_iteration: i32,
    pub rounds: Vec<String>,
}

impl QuizSession {
    pub fn new(game_id: Uuid) -> Self {
        Self {
            game_id,
            current_iteration: 0,
            rounds: vec![],
        }
    }

    pub fn from_game(game: QuizGame) -> Self {
        Self {
            game_id: game.id,
            current_iteration: 0,
            rounds: game.rounds,
        }
    }

    pub fn from_random(game: RandomGame) -> Self {
        Self {
            game_id: game.game_id,
            current_iteration: 0,
            rounds: game.rounds,
        }
    }
}

// ============================================================================
// Spin Game Types (Duel & Roulette)
// ============================================================================

impl GameConverter for SpinSession {
    fn to_json(&self) -> Result<serde_json::Value, serde_json::Error> {
        serde_json::to_value(self)
    }
}

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct SpinGame {
    pub id: Uuid,
    pub rounds: Vec<String>,
}

impl From<SpinSession> for SpinGame {
    fn from(value: SpinSession) -> Self {
        Self {
            id: value.game_id,
            rounds: value.rounds,
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub enum SpinGameState {
    Created,
    Initialized,
    RoundStarted,
    RoundInProgress,
    RoundFinished,
    Finished,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SpinSession {
    pub game_id: Uuid,
    pub host_id: Uuid,
    pub state: SpinGameState,
    pub current_iteration: i32,
    pub selection_size: i32,
    pub rounds: Vec<String>,
    pub players: HashMap<Uuid, i32>,
}

impl SpinSession {
    pub fn new_duel(host_id: Uuid, game_id: Uuid) -> Self {
        Self::from_request(host_id, game_id, 2)
    }

    pub fn new_roulette(host_id: Uuid, game_id: Uuid) -> Self {
        Self::from_request(host_id, game_id, 1)
    }

    fn from_request(host_id: Uuid, game_id: Uuid, selection_size: i32) -> Self {
        Self {
            game_id,
            host_id,
            state: SpinGameState::Created,
            current_iteration: 0,
            selection_size,
            rounds: vec![],
            players: HashMap::from([(host_id, 0)]),
        }
    }

    pub fn from_random_duel(user_id: Uuid, game: RandomGame) -> Self {
        Self::from_random_game(user_id, game, 2)
    }

    pub fn from_random_roulette(user_id: Uuid, game: RandomGame) -> Self {
        Self::from_random_game(user_id, game, 1)
    }

    fn from_random_game(user_id: Uuid, game: RandomGame, selection_size: i32) -> Self {
        Self {
            game_id: game.game_id,
            host_id: user_id,
            state: SpinGameState::Initialized,
            current_iteration: 0,
            selection_size,
            rounds: game.rounds,
            players: HashMap::from([(user_id, 0)]),
        }
    }

    pub fn from_duel(user_id: Uuid, game: SpinGame) -> Self {
        Self::from_game(user_id, game, 2)
    }

    pub fn from_roulette(user_id: Uuid, game: SpinGame) -> Self {
        Self::from_game(user_id, game, 1)
    }

    fn from_game(user_id: Uuid, game: SpinGame, selection_size: i32) -> Self {
        Self {
            game_id: game.id,
            host_id: user_id,
            state: SpinGameState::Initialized,
            current_iteration: 0,
            selection_size,
            rounds: game.rounds,
            players: HashMap::from([(user_id, 0)]),
        }
    }
}

// ============================================================================
// Imposter Game Types
// ============================================================================

impl GameConverter for ImposterSession {
    fn to_json(&self) -> Result<serde_json::Value, serde_json::Error> {
        serde_json::to_value(self)
    }
}

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct ImposterGame {
    pub id: Uuid,
    pub rounds: Vec<String>,
}

impl From<ImposterSession> for ImposterGame {
    fn from(value: ImposterSession) -> Self {
        Self {
            id: value.game_id,
            rounds: value.rounds,
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub enum ImposterGameState {
    Created,
    Initialized,
    Started,
    Finished,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ImposterSession {
    pub game_id: Uuid,
    pub host_id: Uuid,
    pub state: ImposterGameState,
    pub current_iteration: i32,
    pub rounds: Vec<String>,
    pub players: HashMap<Uuid, i32>,
}

impl ImposterSession {
    pub fn new(host_id: Uuid, game_id: Uuid) -> Self {
        Self {
            game_id,
            host_id,
            state: ImposterGameState::Created,
            current_iteration: 0,
            rounds: vec![],
            players: HashMap::from([(host_id, 0)]),
        }
    }

    pub fn from_random(user_id: Uuid, game: RandomGame) -> Self {
        Self {
            game_id: game.game_id,
            host_id: user_id,
            state: ImposterGameState::Initialized,
            current_iteration: 0,
            rounds: game.rounds,
            players: HashMap::from([(user_id, 0)]),
        }
    }
}
