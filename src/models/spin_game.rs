use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::models::game_base::GameConverter;

impl GameConverter for SpinSession {
    fn to_json_value(&self) -> Result<serde_json::Value, serde_json::Error> {
        serde_json::to_value(self)
    }
}

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct SpinGame {
    pub id: Uuid,
    pub rounds: Vec<String>,
}

impl Into<SpinGame> for SpinSession {
    fn into(self) -> SpinGame {
        SpinGame {
            id: self.game_id,
            rounds: self.rounds,
        }
    }
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
            state: SpinGameState::Initialized,
            current_iteration: 0,
            selection_size,
            rounds: vec![],
            players: HashMap::from([(host_id, 0)]),
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
