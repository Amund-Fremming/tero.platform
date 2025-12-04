use sqlx::{Pool, Postgres};

use tracing::error;

use crate::{
    db::system_log::create_system_log,
    models::{
        error::ServerError,
        system_log::{LogAction, LogCeverity, SubjectType},
        user::SubjectId,
    },
};

pub struct SystemLogBuilder {
    pub pool: Pool<Postgres>,
    pub subject_id: Option<String>,
    pub subject_type: Option<SubjectType>,
    pub action: Option<LogAction>,
    pub ceverity: Option<LogCeverity>,
    pub function: Option<String>,
    pub description: Option<String>,
    pub metadata: Option<serde_json::Value>,
}

impl SystemLogBuilder {
    pub fn new(pool: &Pool<Postgres>) -> Self {
        Self {
            pool: pool.clone(),
            subject_id: None,
            subject_type: None,
            action: None,
            ceverity: None,
            function: None,
            description: None,
            metadata: None,
        }
    }

    pub fn subject(mut self, subject: SubjectId) -> Self {
        let (id, _type) = match subject {
            SubjectId::PseudoUser(id) => (id.to_string(), SubjectType::GuestUser),
            SubjectId::BaseUser(id) => (id.to_string(), SubjectType::RegisteredUser),
            SubjectId::Integration(int_name) => (int_name.to_string(), SubjectType::Integration),
        };
        self.subject_id = Some(id);
        self.subject_type = Some(_type);
        self
    }

    pub fn action(mut self, action: LogAction) -> Self {
        self.action = Some(action);
        self
    }

    pub fn ceverity(mut self, ceverity: LogCeverity) -> Self {
        self.ceverity = Some(ceverity);
        self
    }

    pub fn function(mut self, function_name: &str) -> Self {
        self.function = Some(function_name.into());
        self
    }

    pub fn description(mut self, description: &str) -> Self {
        self.description = Some(description.into());
        self
    }

    pub fn metadata(mut self, metadata: serde_json::Value) -> Self {
        self.metadata = Some(metadata);
        self
    }

    pub async fn log(self) -> Result<(), ServerError> {
        let (subject_id, subject_type) = match (self.subject_id, self.subject_type) {
            (Some(id), Some(_type)) => (id, _type),
            _ => ("[SYSTEM]".to_string(), SubjectType::System),
        };

        let mut description = self
            .description
            .unwrap_or_else(|| "No description".to_string());

        // Ensure description fits VARCHAR(512) constraint
        if description.len() > 512 {
            description = format!("{}...", &description[..509]);
        }

        let action = self.action.unwrap_or_else(|| LogAction::Other);
        let ceverity = self.ceverity.unwrap_or_else(|| LogCeverity::Info);
        let function = self.function.unwrap_or_else(|| "Not specified".into());

        create_system_log(
            &self.pool,
            &subject_id,
            &subject_type,
            &action,
            &ceverity,
            &function,
            &description,
            &self.metadata,
        )
        .await?;
        Ok(())
    }

    pub fn log_async(self) {
        tokio::spawn(async move {
            self.log().await.map_err(|e| {
                error!("Failed to system log async: {}", e);
            })
        });
    }
}
