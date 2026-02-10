use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::models::game_base::{GameConverter, RandomGame};

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
