use std::collections::HashMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::models::game_base::{CreateGameRequest, GameCategory, GameConverter};

impl GameConverter for SpinSession {
    fn to_json_value(&self) -> Result<serde_json::Value, serde_json::Error> {
        serde_json::to_value(self)
    }
}

// This does not refelct the db table "spin_game"
#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct SpinGame {
    pub id: Uuid,
    pub rounds: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum SpinGameState {
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
    pub iterations: i32,
    pub current_iteration: i32,
    pub selection_size: i32,
    pub rounds: Vec<String>,
    pub players: HashMap<Uuid, i32>,
}

impl SpinSession {
    pub fn from_create_request69(
        user_id: Uuid,
        selection_size: i32,
        request: CreateGameRequest,
    ) -> Self {
        Self {
            game_id: Uuid::new_v4(),
            host_id: user_id,
            state: SpinGameState::Initialized,
            iterations: 0,
            current_iteration: 0,
            selection_size,
            rounds: vec![],
            players: HashMap::from([(user_id, 0)]),
        }
    }

    pub fn from_duel(user_id: Uuid, game: SpinGame) -> Self {
        Self::from_game(user_id, 2, game)
    }

    pub fn from_roulett(user_id: Uuid, game: SpinGame) -> Self {
        Self::from_game(user_id, 1, game)
    }

    fn from_game(user_id: Uuid, selection_size: i32, game: SpinGame) -> Self {
        Self {
            game_id: game.id,
            host_id: user_id,
            state: SpinGameState::Initialized,
            iterations: game.rounds.len() as i32,
            current_iteration: 0,
            selection_size,
            rounds: game.rounds,
            players: HashMap::from([(user_id, 0)]),
        }
    }
}
