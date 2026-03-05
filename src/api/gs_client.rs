use reqwest::StatusCode;
use serde::{Deserialize, Serialize};
use tracing::error;
use uuid::Uuid;

use crate::models::game_base::{GameType, InitiateGameRequest};

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
    pub hub_name: String,
    pub game_id: Uuid,
    pub is_draft: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct JoinGameResponse {
    pub game_key: String,
    pub hub_name: String,
    pub game_id: Uuid,
    pub game_type: GameType,
    pub is_draft: bool,
}

#[derive(Debug, Clone)]
pub struct GSClient {
    client: reqwest::Client,
    domain: String,
}

impl GSClient {
    pub fn new(domain: impl Into<String>, client: reqwest::Client) -> Self {
        let domain = domain.into();
        Self { domain, client }
    }

    pub async fn health_check(&self) -> Result<(), GSClientError> {
        let response = self
            .client
            .get(format!("{}/health", self.domain))
            .send()
            .await?;
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
        game_type: &GameType,
        payload: &InitiateGameRequest,
    ) -> Result<(), GSClientError> {
        let url = format!("{}/session/initiate/{}", self.domain, game_type.as_str());
        let response = self
            .client
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
