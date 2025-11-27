use core::fmt;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct SystemLog {
    pub id: i64,
    pub subject_id: String,
    pub subject_type: SubjectType,
    pub action: LogAction,
    pub ceverity: LogCeverity,
    pub file_name: String,
    pub description: String,
    pub metadata: Option<serde_json::Value>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "log_ceverity", rename_all = "lowercase")]
pub enum LogCeverity {
    Critical,
    Warning,
    Info,
}

impl fmt::Display for LogCeverity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LogCeverity::Critical => write!(f, "critical"),
            LogCeverity::Warning => write!(f, "warning"),
            LogCeverity::Info => write!(f, "info"),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "log_action", rename_all = "lowercase")]
pub enum LogAction {
    Create,
    Read,
    Update,
    Delete,
    Sync,
    Other,
}

impl fmt::Display for LogAction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LogAction::Create => write!(f, "write"),
            LogAction::Read => write!(f, "read"),
            LogAction::Update => write!(f, "update"),
            LogAction::Delete => write!(f, "delete"),
            LogAction::Sync => write!(f, "sync"),
            LogAction::Other => write!(f, "other"),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "subject_type", rename_all = "lowercase")]
pub enum SubjectType {
    #[sqlx(rename = "registered_user")]
    RegisteredUser,
    #[sqlx(rename = "guest_user")]
    GuestUser,
    Integration,
    System,
}

impl fmt::Display for SubjectType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SubjectType::RegisteredUser => write!(f, "registered"),
            SubjectType::GuestUser => write!(f, "guest"),
            SubjectType::Integration => write!(f, "integration"),
            SubjectType::System => write!(f, "system"),
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SyslogPageQuery {
    pub page_num: Option<u16>,
    pub subject_type: Option<SubjectType>,
    pub action: Option<LogAction>,
    pub ceverity: Option<LogCeverity>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CreateSyslogRequest {
    pub action: Option<LogAction>,
    pub ceverity: Option<LogCeverity>,
    pub description: Option<String>,
    pub file_name: Option<String>,
    pub metadata: Option<serde_json::Value>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct LogCategoryCount {
    pub info: i64,
    pub warning: i64,
    pub critical: i64,
}
