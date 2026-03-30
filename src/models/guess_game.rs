use std::collections::HashSet;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::models::game_base::JsonConverter;

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct GuessGame {
    pub id: Uuid,
    pub rounds: sqlx::types::Json<Vec<GuessRound>>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GuessRound {
    pub prompt: String,
    pub answer: String,
    pub distractors: Vec<String>,
}

impl From<GuessSession> for GuessGame {
    fn from(value: GuessSession) -> Self {
        Self {
            id: value.game_id,
            rounds: value.rounds.into(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GuessSession {
    pub game_id: Uuid,
    pub host_id: Uuid,
    pub current_iteration: i32,
    pub rounds: Vec<GuessRound>,
    pub players: HashSet<String>,
}

impl GuessSession {
    pub fn new(host_id: Uuid, game_id: Uuid) -> Self {
        Self {
            game_id,
            host_id,
            current_iteration: 0,
            rounds: Vec::new(),
            players: HashSet::new(),
        }
    }

    pub fn from_rounds(user_id: Uuid, game_id: Uuid, rounds: Vec<GuessRound>) -> Self {
        Self {
            game_id,
            host_id: user_id,
            current_iteration: 0,
            rounds,
            players: HashSet::new(),
        }
    }
}

impl JsonConverter for GuessSession {
    fn to_json(&self) -> Result<serde_json::Value, serde_json::Error> {
        serde_json::to_value(self)
    }
}
