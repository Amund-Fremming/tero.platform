use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::models::game_base::GameConverter;

impl GameConverter for QuizSession {
    fn to_json_value(&self) -> Result<serde_json::Value, serde_json::Error> {
        serde_json::to_value(self)
    }
}

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct QuizGame {
    pub id: Uuid,
    pub questions: Vec<String>,
}

impl Into<QuizGame> for QuizSession {
    fn into(self) -> QuizGame {
        QuizGame {
            id: self.game_id,
            questions: self.questions,
        }
    }
}

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct QuizSession {
    pub game_id: Uuid,
    pub current_iteration: i32,
    pub questions: Vec<String>,
}

impl QuizSession {
    pub fn new(game_id: Uuid) -> Self {
        Self {
            game_id,
            current_iteration: 0,
            questions: vec![],
        }
    }

    pub fn from_game(game: QuizGame) -> Self {
        Self {
            game_id: game.id,
            current_iteration: 0,
            questions: game.questions,
        }
    }
}
