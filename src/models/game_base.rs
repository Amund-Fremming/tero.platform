use core::fmt;
use std::hash::Hash;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use validator::Validate;

use crate::models::{
    imposter_game::ImposterSession, quiz_game::QuizSession, spin_game::SpinSession,
};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct GamePagedRequest {
    pub page_num: Option<u16>,
    pub game_type: Option<GameType>,
    pub category: Option<GameCategory>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PagedResponse<T> {
    pub page_num: u16,
    pub items: Vec<T>,
    pub has_next: bool,
    pub has_prev: bool,
}

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
    Girls,
    Boys,
    Mixed,
    InnerCircle,
}

impl fmt::Display for GameCategory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GameCategory::Girls => write!(f, "girls"),
            GameCategory::Boys => write!(f, "boys"),
            GameCategory::Mixed => write!(f, "mixed"),
            GameCategory::InnerCircle => write!(f, "innercircle"),
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
    pub fn from_request(query: &GamePagedRequest) -> Self {
        Self {
            page_num: query.page_num.unwrap_or(0),
            game_type: query.game_type.unwrap_or(GameType::Quiz),
            category: query.category.clone(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct InteractiveEnvelope {
    pub payload: serde_json::Value,
}

#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct CreateGameRequest {
    #[validate(custom(function = "crate::api::validation::validate_game_name"))]
    pub name: String,
    pub category: GameCategory,
}

impl CreateGameRequest {
    pub fn new(name: String) -> Self {
        Self {
            name,
            category: GameCategory::Mixed,
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
