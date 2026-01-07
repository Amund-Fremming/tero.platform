use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::models::{game_base::Gender, integration::IntegrationName};

#[derive(Debug, Serialize, Deserialize)]
pub struct ListUsersQuery {
    pub page_num: u8,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct EnsureUserQuery {
    pub pseudo_id: Option<Uuid>,
}

#[derive(Debug, Serialize, Deserialize, Hash, PartialEq, Eq, Clone)]
pub enum Permission {
    #[serde(rename(deserialize = "read:admin"))]
    ReadAdmin,
    #[serde(rename(deserialize = "write:admin"))]
    WriteAdmin,
    #[serde(rename(deserialize = "write:game"))]
    WriteGame,
    #[serde(rename(deserialize = "write:system_log"))]
    WriteSystemLog,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum SubjectId {
    PseudoUser(Uuid),
    BaseUser(Uuid),
    Integration(IntegrationName),
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Auth0User {
    #[serde(rename = "user_id")]
    pub auth0_id: String,
    pub email: Option<String>,
    pub email_verified: Option<bool>,
    pub username: Option<String>,
    pub phone_number: Option<String>,
    pub phone_verified: Option<bool>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub name: Option<String>,
    pub nickname: Option<String>,
    pub given_name: Option<String>,
    pub family_name: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct BaseUser {
    pub id: Uuid,
    pub username: String,
    pub auth0_id: Option<String>,
    pub gender: Gender,
    pub email: Option<String>,
    pub email_verified: Option<bool>,
    pub updated_at: DateTime<Utc>,
    pub family_name: Option<String>,
    pub given_name: Option<String>,
    pub created_at: DateTime<Utc>,
    pub birth_date: Option<NaiveDate>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "role", content = "user")]
pub enum UserRole {
    Admin(BaseUser),
    BaseUser(BaseUser),
}

#[derive(Debug, Serialize, Deserialize, Default, PartialEq)]
pub struct PatchUserRequest {
    pub username: Option<String>,
    pub gender: Option<Gender>,
    pub family_name: Option<String>,
    pub given_name: Option<String>,
    pub birth_date: Option<NaiveDate>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ActivityStats {
    pub total_game_count: i64,
    pub total_user_count: i64,
    pub recent: RecentUserStats,
    pub average: AverageUserStats,
}

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct RecentUserStats {
    pub this_month_users: i64,
    pub this_week_users: i64,
    pub todays_users: i64,
}

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct AverageUserStats {
    pub avg_month_users: f64,
    pub avg_week_users: f64,
    pub avg_daily_users: f64,
}
