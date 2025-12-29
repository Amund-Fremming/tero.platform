use core::fmt;
use std::hash::Hash;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::models::{quiz_game::QuizSession, spin_game::SpinSession};

pub trait GameConverter {
    fn to_json_value(&self) -> Result<serde_json::Value, serde_json::Error>;
}

#[derive(Serialize)]
#[serde(untagged)]
pub enum JsonWrapper {
    QuizWrapper(QuizSession),
    SpinWrapper(SpinSession),
}

#[derive(Debug, Serialize, Deserialize, Clone, sqlx::FromRow)]
pub struct GameBase {
    pub id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub game_type: GameType,
    pub category: GameCategory,
    pub iterations: i32,
    pub times_played: i32,
    pub last_played: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize, Hash, Clone, sqlx::Type)]
#[sqlx(type_name = "game_category", rename_all = "lowercase")]
pub enum GameCategory {
    Casual,
    Random,
    Ladies,
    Boys,
    Default,
}

impl fmt::Display for GameCategory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GameCategory::Casual => write!(f, "casual"),
            GameCategory::Ladies => write!(f, "ladies"),
            GameCategory::Boys => write!(f, "boys"),
            GameCategory::Default => write!(f, "default"),
            GameCategory::Random => write!(f, "random"),
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

#[derive(Debug, Serialize, Deserialize, Hash, Clone, sqlx::Type)]
#[sqlx(type_name = "game_type", rename_all = "lowercase")]
pub enum GameType {
    Roulette,
    Duel,
    Quiz,
}

impl GameType {
    pub fn table_name(&self) -> &'static str {
        match self {
            GameType::Quiz => "quiz_game",
            GameType::Duel | GameType::Roulette => "spin_game",
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            GameType::Quiz => "quiz",
            GameType::Duel => "duel",
            GameType::Roulette => "spin",
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Hash)]
pub struct GamePageQuery {
    pub page_num: u16,
    pub game_type: GameType,
    pub category: Option<GameCategory>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SavedGamesPageQuery {
    pub page_num: u8,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct InteractiveEnvelope {
    pub payload: serde_json::Value,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CreateGameRequest {
    pub name: String,
    pub description: Option<String>,
    pub category: Option<GameCategory>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct InitiateGameRequest {
    pub key: String,
    pub value: serde_json::Value,
}
