use std::{collections::HashSet, time::SystemTimeError};

use axum::{http::StatusCode, response::IntoResponse};
use thiserror::Error;
use tracing::{error, warn};

use crate::{
    api::gs_client::GSClientError, models::user::Permission, service::key_vault::KeyVaultError,
};

#[derive(Debug, Error)]
pub enum ServerError {
    #[error("Sqlx failed: {0}")]
    Sqlx(#[from] sqlx::Error),

    #[error("Internal server error: {0}")]
    Internal(String),

    #[error("Api error: {1}")]
    Api(StatusCode, String),

    #[error("Permission error")]
    Permission(HashSet<Permission>),

    #[error("Access denied error")]
    AccessDenied,

    #[error("Not found: {0}")]
    NotFound(String),

    #[error("Request error: {0}")]
    Reqwest(#[from] reqwest::Error),

    #[error("JWT verification error: {0}")]
    JwtVerification(String),

    #[error("Json error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("GSClient error: {0}")]
    GSClientError(#[from] GSClientError),

    #[error("KeyVault error: {0}")]
    KeyVaultError(#[from] KeyVaultError),

    #[error("Failed to create system time: {0}")]
    TimeCreation(#[from] SystemTimeError),
}

impl IntoResponse for ServerError {
    fn into_response(self) -> axum::response::Response {
        match self {
            ServerError::Sqlx(e) => {
                error!("Sqlx failed with error: {:?}", e);
                (StatusCode::INTERNAL_SERVER_ERROR, String::new())
            }
            ServerError::Internal(e) => {
                error!("Internal server error: {}", e);
                (StatusCode::INTERNAL_SERVER_ERROR, String::new())
            }
            ServerError::Api(sc, msg) => {
                error!("Api error: {} - {}", sc, msg);
                (sc, msg)
            }
            ServerError::Permission(missing) => {
                warn!("Missing permission: {:?}", missing);
                (
                    StatusCode::FORBIDDEN,
                    format!("Missing permission: {:?}", missing),
                )
            }
            ServerError::NotFound(e) => {
                warn!("Entity not found: {}", e);
                (StatusCode::NOT_FOUND, e)
            }
            ServerError::AccessDenied => {
                warn!("Access denied for requesting entity");
                (StatusCode::FORBIDDEN, String::from("Access denied"))
            }
            ServerError::Reqwest(e) => {
                error!("Failed to send request: {}", e);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    String::from("Failed to access third party"),
                )
            }
            ServerError::JwtVerification(e) => {
                warn!("Failed to verify JWT: {}", e);
                (StatusCode::UNAUTHORIZED, String::new())
            }
            ServerError::Json(e) => {
                error!("Json error: {}", e);
                (StatusCode::INTERNAL_SERVER_ERROR, String::new())
            }
            ServerError::GSClientError(e) => {
                error!("GSClient error: {}", e);
                (
                    StatusCode::SERVICE_UNAVAILABLE,
                    String::from("Upstream service unavailable"),
                )
            }
            ServerError::KeyVaultError(e) => {
                error!("KeyVault error: {}", e);
                (StatusCode::INTERNAL_SERVER_ERROR, String::new())
            }
            ServerError::TimeCreation(e) => {
                error!("Failed to create system time: {:?}", e);
                (StatusCode::INTERNAL_SERVER_ERROR, String::new())
            }
        }
        .into_response()
    }
}
