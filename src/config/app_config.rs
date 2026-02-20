use core::fmt;
use std::env;

use config::{Config, ConfigError, Environment, File};
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use tracing::debug;

use crate::models::integration::IntegrationConfig;

pub static CONFIG: Lazy<AppConfig> =
    Lazy::new(|| AppConfig::load().unwrap_or_else(|e| panic!("{}", e)));

#[derive(Serialize, Deserialize, Debug)]
pub enum Runtime {
    Dev,
    Prod,
}

impl fmt::Display for Runtime {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Runtime::Dev => write!(f, "development"),
            Runtime::Prod => write!(f, "production"),
        }
    }
}

impl From<String> for Runtime {
    fn from(value: String) -> Self {
        match value.as_str() {
            "DEVELOPMENT" => Runtime::Dev,
            "PRODUCTION" => Runtime::Prod,
            _ => Runtime::Prod,
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AppConfig {
    pub server: ServerConfig,
    pub auth0: Auth0Config,
    pub database_url: String,
    pub integrations: Vec<IntegrationConfig>,
}

fn default_address() -> String {
    "127.0.0.1".into()
}

fn default_port() -> String {
    "3000".into()
}

fn default_page_size() -> u16 {
    20
}

fn default_runtime() -> Runtime {
    Runtime::Dev
}

fn default_active_game_retention() -> u8 {
    24
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ServerConfig {
    #[serde(default = "default_address")]
    pub address: String,
    #[serde(default = "default_port")]
    pub port: String,
    pub gs_domain: String,
    #[serde(default = "default_page_size")]
    pub page_size: u16,
    #[serde(default = "default_active_game_retention")]
    pub active_game_retention: u8,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Auth0Config {
    pub domain: String,
    pub audience: String,
    pub client_id: String,
    pub webhook_key: String,
    #[serde(default = "default_runtime")]
    pub runtime: Runtime,
}

impl AppConfig {
    fn load() -> Result<Self, ConfigError> {
        let runtime: Runtime = env::var("ENVIRONMENT").expect("ENVIRONMENT not set").into();

        let config: AppConfig = Config::builder()
            .add_source(File::with_name(&format!("src/config/{}.toml", runtime)))
            .add_source(Environment::with_prefix("TERO").separator("__"))
            .build()?
            .try_deserialize()?;

        debug!(
            "Loaded config: {}",
            serde_json::to_string_pretty(&config).unwrap()
        );

        Ok(config)
    }
}
