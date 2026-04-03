use std::sync::Arc;

use chrono::{Datelike, TimeZone, Utc};
use chrono_tz::Europe::Oslo;
use serde_json::json;

use reqwest::Client;
use sqlx::{Pool, Postgres};
use tokio::sync::{Mutex, mpsc};
use tracing::{debug, error, info, warn};
use uuid::Uuid;

use crate::{
    api::gs_client::GSClient,
    config::app_config::CONFIG,
    db::{
        game_base::{delete_stale_games, fill_rounds_pool},
        guess_game::get_guess_game_by_id,
        imposter_game::get_imposter_game_by_id,
        quiz_game::get_quiz_game_by_id,
        spin_game::get_spin_game_by_id,
    },
    models::{
        auth::Jwks,
        error::ServerError,
        game_base::{GameBase, GameType, PagedResponse},
        system_log::{LogAction, LogCeverity},
    },
    service::{
        cache::GustCache, key_vault::KeyVault, popup_manager::PopupManager,
        system_log_builder::SystemLogBuilder,
    },
};

type RoundPoolSender = Arc<Mutex<Option<mpsc::UnboundedSender<(Uuid, GameType)>>>>;

#[derive(Clone)]
pub struct AppState {
    pool: Pool<Postgres>,
    jwks: Jwks,
    client: Client,
    gs_client: GSClient,
    page_cache: Arc<GustCache<PagedResponse<GameBase>>>,
    key_vault: Arc<KeyVault>,
    popup_manager: PopupManager,

    /// Channel used to queue up a new game to write its rounds to the round pool
    round_pool_sender: RoundPoolSender,
}

impl AppState {
    pub async fn from_pool(pool: Pool<Postgres>) -> Result<Arc<Self>, ServerError> {
        let client = Client::new();
        let gs_client = GSClient::new(&CONFIG.server.gs_domain, client.clone());

        let jwks_url = format!("{}.well-known/jwks.json", CONFIG.auth0.domain);
        let response = client.get(jwks_url).send().await?;
        let jwks = response.json::<Jwks>().await?;
        let page_cache = Arc::new(GustCache::from_ttl(120));
        let key_vault = Arc::new(KeyVault::load_words(&pool).await?);
        let popup_manager = PopupManager::new();
        let round_pool_sender = Arc::new(Mutex::new(None));

        Ok(Arc::new(Self {
            pool,
            jwks,
            client,
            gs_client,
            page_cache,
            key_vault,
            popup_manager,
            round_pool_sender,
        }))
    }

    #[cfg(test)]
    pub async fn from_connection_string(connection_string: &str) -> Result<Arc<Self>, ServerError> {
        let pool = Pool::<Postgres>::connect(connection_string).await?;
        Self::from_pool(pool).await
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

    pub async fn fill_rounds_pool(&self, game_id: Uuid, game_type: GameType) {
        let lock = self.round_pool_sender.lock().await;
        let Some(sender) = lock.as_ref() else {
            return;
        };

        if let Err(e) = sender.send((game_id, game_type)) {
            error!(
                "Failed to enqueue round pool job for game {} ({}): {}",
                game_id,
                game_type.as_str(),
                e
            );

            SystemLogBuilder::new(self.get_pool())
                .action(LogAction::Other)
                .ceverity(LogCeverity::Warning)
                .function("fill_rounds_pool")
                .description("Failed to enqueue round pool background job")
                .metadata(json!({
                    "game_id": game_id,
                    "game_type": game_type.as_str(),
                    "error": e.to_string()
                }))
                .log_async();
        };
    }

    pub fn spawn_round_pool_job(&self) {
        let pool = self.get_pool().clone();
        let sender = self.round_pool_sender.clone();

        tokio::spawn(async move {
            loop {
                let worker_pool = pool.clone();
                let worker_sender = sender.clone();

                let worker = tokio::spawn(async move {
                    Self::run_round_pool_supervisor(&worker_pool, worker_sender).await;
                });

                match worker.await {
                    Ok(()) => {
                        warn!("Round pool supervisor task exited; restarting");
                        SystemLogBuilder::new(&pool)
                            .action(LogAction::Other)
                            .ceverity(LogCeverity::Info)
                            .function("spawn_round_pool_job")
                            .description("Round pool supervisor task exited and is being restarted")
                            .log_async();
                    }
                    Err(e) => {
                        error!("Round pool supervisor task panicked: {}", e);
                        SystemLogBuilder::new(&pool)
                            .action(LogAction::Other)
                            .ceverity(LogCeverity::Warning)
                            .function("spawn_round_pool_job")
                            .description(
                                "Round pool supervisor task panicked and is being restarted",
                            )
                            .metadata(json!({"error": e.to_string(), "panic": e.is_panic()}))
                            .log_async();
                    }
                }

                info!("Restarting round pool supervisor task");
            }
        });
    }

    async fn run_round_pool_supervisor(pool: &Pool<Postgres>, sender: RoundPoolSender) {
        loop {
            if let Err(e) = Self::init_channels(pool, sender.clone()).await {
                error!("Round pool bg job failed: {}", e);
                info!("Restarting round pool bg job");

                if let Err(e) = SystemLogBuilder::new(pool)
                    .action(LogAction::Other)
                    .ceverity(LogCeverity::Warning)
                    .function("run_round_pool_supervisor")
                    .description("Round pool worker failed and is being restarted")
                    .metadata(json!({"error": e.to_string()}))
                    .log()
                    .await
                {
                    error!("Round pool job failed to create audit log: {}", e);
                };
            }
        }
    }

    async fn init_channels(
        pool: &Pool<Postgres>,
        sender: RoundPoolSender,
    ) -> Result<(), ServerError> {
        let (new_sender, mut new_receiver) = mpsc::unbounded_channel();

        {
            let mut sender_lock = sender.lock().await;
            *sender_lock = Some(new_sender);
        }

        info!("Round pool worker active and listening...");

        while let Some((game_id, game_type)) = new_receiver.recv().await {
            debug!(
                "Processing {} rounds into 'round_pool' for game: {}",
                game_type.as_str(),
                game_id
            );
            match game_type {
                GameType::Roulette | GameType::Duel => {
                    let game = match get_spin_game_by_id(pool, game_id).await {
                        Ok(game) => game,
                        Err(e) => {
                            error!("Round pool bg job failed to get spin game: {}", e);
                            continue;
                        }
                    };

                    if let Err(e) = fill_rounds_pool(pool, game_type, game.rounds).await {
                        error!("Round pool bg job failed to fill rounds: {}", e);
                        continue;
                    }
                }
                GameType::Quiz => {
                    let game = match get_quiz_game_by_id(pool, game_id).await {
                        Ok(game) => game,
                        Err(e) => {
                            error!("Round pool bg job failed to get quiz game: {}", e);
                            continue;
                        }
                    };

                    if let Err(e) = fill_rounds_pool(pool, game_type, game.rounds).await {
                        error!("Round pool bg job failed to fill rounds: {}", e);
                        continue;
                    }
                }
                GameType::Imposter => {
                    let game = match get_imposter_game_by_id(pool, game_id).await {
                        Ok(game) => game,
                        Err(e) => {
                            error!("Round pool bg job failed to get imposter game: {}", e);
                            continue;
                        }
                    };

                    if let Err(e) = fill_rounds_pool(pool, game_type, game.rounds).await {
                        error!("Round pool bg job failed to fill rounds: {}", e);
                        continue;
                    }
                }
                GameType::Guess => {
                    let game = match get_guess_game_by_id(pool, game_id).await {
                        Ok(game) => game,
                        Err(e) => {
                            error!("Round pool bg job failed to get guess game: {}", e);
                            continue;
                        }
                    };

                    if let Err(e) = fill_rounds_pool(pool, game_type, game.rounds.0).await {
                        error!("Round pool bg job failed to fill rounds: {}", e);
                        continue;
                    }
                }
            };
        }

        Ok(())
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
