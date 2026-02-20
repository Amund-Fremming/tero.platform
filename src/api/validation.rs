use axum::{Json, extract::FromRequest};
use reqwest::StatusCode;
use serde::de::DeserializeOwned;
use tracing::{debug, info};
use validator::{Validate, ValidationError};

use crate::models::error::ServerError;

#[derive(Debug)]
pub struct ValidatedJson<T>(pub T);

impl<T, S> FromRequest<S> for ValidatedJson<T>
where
    T: DeserializeOwned + Validate + Send + 'static,
    S: Send + Sync,
{
    type Rejection = ServerError;

    async fn from_request(req: axum::extract::Request, state: &S) -> Result<Self, Self::Rejection> {
        let content_type = req
            .headers()
            .get(axum::http::header::CONTENT_TYPE)
            .and_then(|h| h.to_str().ok())
            .ok_or_else(|| ServerError::Api(StatusCode::BAD_REQUEST, "Invalid JSON".to_string()))?;

        let value = if content_type.starts_with("application/json") {
            match Json::<T>::from_request(req, state).await {
                Ok(Json(val)) => val,
                Err(_) => {
                    return Err(ServerError::Api(
                        StatusCode::BAD_REQUEST,
                        "Invalid JSON".into(),
                    ));
                }
            }
        } else {
            return Err(ServerError::Api(
                StatusCode::BAD_REQUEST,
                "Expected JSON".to_string(),
            ));
        };

        match value.validate() {
            Ok(_) => {
                debug!("Validation passed");
                Ok(ValidatedJson(value))
            }
            Err(e) => {
                let error_msg = format_validation_errors(&e);
                info!("Validation error: {}", error_msg);
                Err(ServerError::Api(StatusCode::BAD_REQUEST, error_msg))
            }
        }
    }
}

/// Format validation errors into a user-friendly message
fn format_validation_errors(errors: &validator::ValidationErrors) -> String {
    let mut messages = Vec::new();

    for (field, field_errors) in errors.field_errors() {
        for error in field_errors {
            let msg = error
                .message
                .as_ref()
                .map(|m| m.to_string())
                .unwrap_or_else(|| format!("{} validation failed", field));
            messages.push(msg);
        }
    }

    if messages.is_empty() {
        "Validation failed".to_string()
    } else {
        messages.join(", ")
    }
}

// Validation functions for reuse across models

/// Validate username: 3-30 chars, alphanumeric, underscores, hyphens, periods (but not at start)
pub fn validate_username(username: &str) -> Result<(), ValidationError> {
    let len = username.len();

    if len < 3 {
        return Err(ValidationError::new("username_too_short")
            .with_message("Username must be at least 3 characters".into()));
    }

    if len > 30 {
        return Err(ValidationError::new("username_too_long")
            .with_message("Username must be at most 30 characters".into()));
    }

    if username.starts_with('.') {
        return Err(ValidationError::new("username_invalid_start")
            .with_message("Username cannot start with a period".into()));
    }

    if !username
        .chars()
        .all(|c| c.is_alphanumeric() || c == '_' || c == '-' || c == '.')
    {
        return Err(ValidationError::new("username_invalid_chars").with_message(
            "Username can only contain letters, numbers, underscores, hyphens and periods".into(),
        ));
    }

    Ok(())
}

/// Validate person name (given_name, family_name): 1-50 chars, letters, spaces, and common name chars
pub fn validate_person_name(name: &str) -> Result<(), ValidationError> {
    let len = name.trim().len();

    if len == 0 {
        return Err(ValidationError::new("name_empty").with_message("Name cannot be empty".into()));
    }

    if len > 50 {
        return Err(ValidationError::new("name_too_long")
            .with_message("Name must be at most 50 characters".into()));
    }

    if !name
        .chars()
        .all(|c| c.is_alphabetic() || c.is_whitespace() || c == '\'' || c == '-' || c == '.')
    {
        return Err(ValidationError::new("name_invalid_chars").with_message(
            "Name can only contain letters, spaces, hyphens, apostrophes and periods".into(),
        ));
    }

    Ok(())
}

/// Validate game name: 3-100 chars, not just whitespace
pub fn validate_game_name(name: &str) -> Result<(), ValidationError> {
    let trimmed = name.trim();
    let len = trimmed.len();

    if len < 3 {
        return Err(ValidationError::new("game_name_too_short")
            .with_message("Game name must be at least 3 characters".into()));
    }

    if len > 100 {
        return Err(ValidationError::new("game_name_too_long")
            .with_message("Game name must be at most 100 characters".into()));
    }

    // Check if it's not just whitespace/special chars
    if !trimmed.chars().any(|c| c.is_alphanumeric()) {
        return Err(ValidationError::new("game_name_invalid")
            .with_message("Game name must contain at least one letter or number".into()));
    }

    Ok(())
}
