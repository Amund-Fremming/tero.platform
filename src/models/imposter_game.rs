use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::models::game_base::{GameConverter, RandomGame};

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
    RoundStarted,
    RoundInProgress,
    RoundFinished,
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
