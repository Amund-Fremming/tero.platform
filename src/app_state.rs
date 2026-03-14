use std::sync::Arc;

use chrono::{Datelike, TimeZone, Utc};
use chrono_tz::Europe::Oslo;
use serde_json::json;

use reqwest::Client;
use sqlx::{Pool, Postgres};
use tracing::{info, warn};

use crate::{
    api::gs_client::GSClient,
    config::app_config::CONFIG,
    db::game_base::delete_stale_games,
    models::{
        auth::Jwks,
        error::ServerError,
        game_base::{GameBase, PagedResponse},
        system_log::{LogAction, LogCeverity},
    },
    service::{
        cache::GustCache, key_vault::KeyVault, popup_manager::PopupManager,
        system_log_builder::SystemLogBuilder,
    },
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
        let pool = Pool::<Postgres>::connect(connection_string).await?;
        let client = Client::new();
        let gs_client = GSClient::new(&CONFIG.server.gs_domain, client.clone());

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

        tokio::spawn(async move {
            loop {
                let delay_secs = secs_until_0500_oslo();
                tokio::time::sleep(std::time::Duration::from_secs(delay_secs)).await;

                let retention_days = CONFIG.server.active_game_retention;
                match delete_stale_games(&pool, retention_days).await {
                    Ok(n) => info!(
                        "Game cleanup: purged {} stale game(s) (retention {}d)",
                        n, retention_days
                    ),
                    Err(e) => {
                        warn!("Game cleanup failed: {}", e);
                        let _ = SystemLogBuilder::new(&pool)
                            .action(LogAction::Delete)
                            .ceverity(LogCeverity::Warning)
                            .function("spawn_game_cleanup")
                            .description("Failed to purge stale games from database")
                            .metadata(json!({"error": e.to_string()}))
                            .log()
                            .await;
                    }
                }
            }
        });
    }
}

fn secs_until_0500_oslo() -> u64 {
    let now_utc = Utc::now();
    let now_oslo = now_utc.with_timezone(&Oslo);

    let today_0500 = Oslo
        .with_ymd_and_hms(now_oslo.year(), now_oslo.month(), now_oslo.day(), 5, 0, 0)
        .earliest()
        .expect("05:00 Oslo is always a valid time");

    let next_run = if now_oslo < today_0500 {
        today_0500
    } else {
        let tomorrow = now_oslo.date_naive().succ_opt().expect("date overflow");
        Oslo.with_ymd_and_hms(tomorrow.year(), tomorrow.month(), tomorrow.day(), 5, 0, 0)
            .earliest()
            .expect("05:00 Oslo tomorrow is always a valid time")
    };

    next_run.signed_duration_since(now_utc).num_seconds().max(0) as u64
}
