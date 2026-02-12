use reqwest::{Client, StatusCode};
use serde::{Deserialize, Serialize};
use tracing::error;

use crate::features::game::models::{GameType, InitiateGameRequest};

#[derive(Debug, thiserror::Error)]
pub enum GSClientError {
    #[error("Http request failed: {0}")]
    Http(#[from] reqwest::Error),

    #[error("Api error: {0} - {1}")]
    ApiError(StatusCode, String),

    #[error("Failed to serialize object: {0}")]
    Serialize(#[from] serde_json::Error),
}

#[derive(Debug, Serialize, Deserialize)]
pub struct InteractiveGameResponse {
    pub key: String,
    pub hub_address: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct JoinGameResponse {
    pub game_key: String,
    pub hub_address: String,
    pub game_type: GameType,
}

#[derive(Debug, Clone)]
pub struct GSClient {
    domain: String,
}

impl GSClient {
    pub fn new(domain: impl Into<String>) -> Self {
        let domain = domain.into();
        Self { domain }
    }

    pub async fn health_check(&self, client: &Client) -> Result<(), GSClientError> {
        let response = client.get(format!("{}/health", self.domain)).send().await?;
        if !response.status().is_success() {
            return Err(GSClientError::ApiError(
                StatusCode::SERVICE_UNAVAILABLE,
                "Failed to reach game session microservice".into(),
            ));
        }

        Ok(())
    }

    pub async fn initiate_game_session(
        &self,
        client: &Client,
        game_type: &GameType,
        payload: &InitiateGameRequest,
    ) -> Result<(), GSClientError> {
        let url = format!("{}/session/initiate/{}", self.domain, game_type.as_str());
        let response = client
            .post(&url)
            .header("content-type", "application/json")
            .json(&payload)
            .send()
            .await?;

        let status = response.status();
        if !status.is_success() {
            let body = response.text().await.unwrap_or("No response body".into());
            error!("GSClient request failed: {} - {}", status, body);
            return Err(GSClientError::ApiError(status, body));
        }

        Ok(())
    }
}
