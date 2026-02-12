use axum::http::HeaderMap;
use reqwest::StatusCode;
use uuid::Uuid;

use crate::common::error::ServerError;

pub fn to_uuid(value: &str) -> Result<Uuid, ServerError> {
    let Ok(guest_id) = value.parse() else {
        return Err(ServerError::Api(
            StatusCode::UNAUTHORIZED,
            "Guest id is invalid".into(),
        ));
    };
    Ok(guest_id)
}

pub fn extract_header(key: &str, header_map: &HeaderMap) -> Option<String> {
    header_map
        .get(key)
        .and_then(|header| header.to_str().ok())
        .map(|s| s.to_owned())
}
