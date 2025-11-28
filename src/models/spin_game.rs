use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::models::game_base::{CreateGameRequest, GameCategory, GameConverter};

impl GameConverter for SpinSession {
    fn to_json_value(&self) -> Result<serde_json::Value, serde_json::Error> {
        serde_json::to_value(self)
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SpinGamePlayer {
    pub user_id: Uuid,
    pub times_chosen: u8,
}

// This does not refelct the db table "spin_game"
#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct SpinGame {
    pub spin_id: Uuid,
    pub base_id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub category: GameCategory,
    pub iterations: i32,
    pub times_played: i32,
    pub last_played: DateTime<Utc>,
    pub rounds: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SpinSession {
    pub spin_id: Uuid,
    pub base_id: Uuid,
    pub host_id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub category: GameCategory,
    pub iterations: i32,
    pub times_played: i32,
    pub last_played: DateTime<Utc>,
    pub rounds: Vec<String>,
    pub players: Vec<SpinGamePlayer>,
}


#[derive(Debug, Serialize, Deserialize)]
pub enum SpinGameState {
    Initialized,
    Started
}

impl SpinSession {
    pub fn from_create_request(user_id: Uuid, request: CreateGameRequest) -> Self {
        let player = SpinGamePlayer {
            user_id,
            times_chosen: 0,
        };

        Self {
            spin_id: Uuid::new_v4(),
            base_id: Uuid::new_v4(),
            host_id: user_id,
            name: request.name,
            description: request.description,
            category: request.category.unwrap_or_else(|| GameCategory::Default),
            iterations: 0,
            times_played: 0,
            last_played: Utc::now(),
            rounds: vec![],
            players: vec![player],
        }
    }

    pub fn from_game(user_id: Uuid, game: SpinGame) -> Self {
        let player = SpinGamePlayer {
            user_id,
            times_chosen: 0,
        };

        Self {
            spin_id: game.spin_id,
            base_id: game.base_id,
            host_id: user_id,
            name: game.name,
            description: game.description,
            category: game.category,
            iterations: game.iterations,
            times_played: game.times_played,
            last_played: game.last_played,
            rounds: game.rounds,
            players: vec![player],
        }
    }
}
