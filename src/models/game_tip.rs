use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use validator::Validate;

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct GameTip {
    pub id: Uuid,
    pub header: String,
    pub mobile_phone: String,
    pub description: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct CreateGameTipRequest {
    #[validate(length(min = 1, max = 100))]
    pub header: String,
    #[validate(length(min = 1, max = 20))]
    pub mobile_phone: String,
    #[validate(length(min = 1, max = 300))]
    pub description: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GameTipPageQuery {
    pub page_num: u16,
}
