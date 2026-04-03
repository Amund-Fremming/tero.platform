use core::fmt;
use std::collections::HashMap;

use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};

use crate::config::app_config::CONFIG;

pub static INTEGRATION_NAMES: Lazy<HashMap<String, IntegrationName>> = Lazy::new(|| {
    CONFIG
        .integrations
        .iter()
        .map(|i| (i.subject.clone(), i.name.clone()))
        .collect()
});

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct IntegrationConfig {
    pub name: IntegrationName,
    pub subject: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, Hash, PartialEq, Eq, sqlx::Type)]
#[sqlx(type_name = "integration_name", rename_all = "lowercase")]
#[serde(rename_all = "lowercase")]
pub enum IntegrationName {
    Auth0,
    Session,
}

impl fmt::Display for IntegrationName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            IntegrationName::Auth0 => write!(f, "auth0"),
            IntegrationName::Session => write!(f, "game_session"),
        }
    }
}

impl IntegrationName {
    pub fn from_subject(
        subject: &str,
        integrations: &HashMap<String, IntegrationName>,
    ) -> Option<IntegrationName> {
        let stripped = subject.strip_suffix("@clients")?;
        integrations.get(stripped).cloned()
    }
}
