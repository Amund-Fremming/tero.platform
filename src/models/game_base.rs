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
}

impl GameBase {
    pub fn new(
        id: Uuid,
        name: String,
        game_type: GameType,
        category: GameCategory,
        iterations: i32,
    ) -> Self {
        Self {
            id,
            name,
            game_type,
            category,
            iterations,
            times_played: 1, // If the user manages to get to the create screen the game has been played 1 time.
            last_played: Utc::now(),
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
    pub game_type: Option<GameType>,
    pub category: Option<GameCategory>,
}

impl GameCacheKey {
    pub fn from_request(query: &GamePagedRequest) -> Self {
        Self {
            page_num: query.page_num.unwrap_or(0),
            game_type: query.game_type,
            category: query.category.clone(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct CreateStaticGameRequest {
    #[validate(custom(function = "crate::api::validation::validate_game_name"))]
    pub name: String,
    pub category: GameCategory,
    pub rounds: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct InteractiveGameEnvelope {
    #[validate(custom(function = "crate::api::validation::validate_game_name"))]
    pub name: String,
    pub category: GameCategory,
    pub payload: serde_json::Value,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct InitiateGameRequest {
    pub key: String,
    pub value: serde_json::Value,
}
