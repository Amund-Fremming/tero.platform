use reqwest::{Client, StatusCode};
use serde::{Deserialize, Serialize};
use tracing::{error, info};

use crate::models::game_base::{GameType};

#[derive(Debug, thiserror::Error)]
pub enum GSClientError {
    #[error("The game is full")]
    Full,

    #[error("The game has started")]
    Started,

    #[error("Http request failed: {0}")]
    Http(#[from] reqwest::Error),

    #[error("Api error: {0} - {1}")]
    ApiError(StatusCode, String),

    #[error("Failed to serialize object: {0}")]
    Serialize(#[from] serde_json::Error),
}

#[derive(Debug, Serialize, Deserialize)]
pub struct InteractiveGameResponse {
    pub game_key: String,
    pub hub_address: String,
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
        let response = client.get(format!("{}health", self.domain)).send().await?;
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
        key: &str,
        session: impl Serialize,
    ) -> Result<(), GSClientError> {
        let uri = format!("{}session/initiate/{}/{}", self.domain, game_type.column_name(), key);
        self.send_json(client, &uri, session).await
    }

    async fn send_json<T: Serialize>(
        &self,
        client: &Client,
        uri: &str,
        body: T,
    ) -> Result<(), GSClientError> {
        info!("GSClient sending request to: {}", uri);
        let url = format!("{}/{}", self.domain, uri);
        let response = client
            .post(&url)
            .header("content-type", "application/json")
            .json(&body)
            .send()
            .await?;

        let status = response.status();
        let body = response.text().await.unwrap_or("No body".into());
        if !status.is_success() {
            error!("GSClient request failed: {} - {}", status, body);
            return Err(GSClientError::ApiError(status, body));
        }

        Ok(())
    }
}
