use std::{sync::Arc, time::Duration};

use serde_json::json;

use reqwest::Client;
use sqlx::{Pool, Postgres};

use crate::{
    api::gs_client::GSClient, config::config::CONFIG, db::game_base::delete_non_active_games, models::{
        auth::Jwks,
        error::ServerError,
        game_base::GameBase,
        system_log::{LogAction, LogCeverity},
    }, service::{cache::GustCache, key_vault::KeyVault, system_log_builder::SystemLogBuilder}
};

#[derive(Clone)]
pub struct AppState {
    pool: Pool<Postgres>,
    jwks: Jwks,
    client: Client,
    gs_client: GSClient,
    page_cache: Arc<GustCache<PagedResponse<GameBase>>>,
    key_vault: Arc<KeyVault>,
    popup_manager: PopupManager,
}

impl AppState {
    pub async fn from_connection_string(connection_string: &str) -> Result<Arc<Self>, ServerError> {
        let pool = Pool::<Postgres>::connect(&connection_string).await?;
        let client = Client::new();
        let gs_client = GSClient::new(&CONFIG.server.gs_domain);

        let jwks_url = format!("{}.well-known/jwks.json", CONFIG.auth0.domain);
        let response = client.get(jwks_url).send().await?;
        let jwks = response.json::<Jwks>().await?;
        let page_cache = Arc::new(GustCache::from_ttl(120));
        let key_vault = Arc::new(KeyVault::load_words(&pool).await?);
        let popup_manager = PopupManager::new();

        let state = Arc::new(Self {
            pool,
            jwks,
            client,
            gs_client,
            page_cache,
            key_vault,
            popup_manager,
        });

        Ok(state)
    }

    pub fn get_pool(&self) -> &Pool<Postgres> {
        &self.pool
    }

    pub fn get_jwks(&self) -> &Jwks {
        &self.jwks
    }

    pub fn get_cache(&self) -> &Arc<GustCache<PagedResponse<GameBase>>> {
        &self.page_cache
    }

    pub fn get_client(&self) -> &Client {
        &self.client
    }

    pub fn get_gs_client(&self) -> &GSClient {
        &self.gs_client
    }

    pub fn syslog(&self) -> SystemLogBuilder {
        SystemLogBuilder::new(self.get_pool())
    }

    pub fn get_vault(&self) -> &KeyVault {
        &self.key_vault
    }

    pub fn get_popup_manager(&self) -> &PopupManager {
        &self.popup_manager
    }

    pub fn spawn_game_cleanup(&self) {
        let pool = self.get_pool().clone();
        let mut interval = tokio::time::interval(Duration::from_secs(86_400));

        tokio::spawn(async move {
            loop {
                interval.tick().await;
                if let Err(e) = delete_non_active_games(&pool).await {
                    let _ = SystemLogBuilder::new(&pool)
                        .action(LogAction::Delete)
                        .ceverity(LogCeverity::Info)
                        .description("Failed to purge inactive games")
                        .metadata(json!({"error": e.to_string()}))
                        .log()
                        .await;
                }
            }
        });
    }
}
