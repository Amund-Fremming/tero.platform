use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct GameTip {
    pub id: Uuid,
    pub header: String,
    pub mobile_phone: String,
    pub description: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CreateGameTipRequest {
    pub header: String,
    pub mobile_phone: String,
    pub description: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GameTipPageQuery {
    pub page_num: u16,
}
