use std::{
    sync::Arc,
    time::{Duration, SystemTime, SystemTimeError, UNIX_EPOCH},
};

use dashmap::DashMap;
use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha8Rng;
use sqlx::{Pool, Postgres};
use tracing::{debug, warn};

use crate::features::game::models::GameType;

pub async fn get_word_sets(
    pool: &Pool<Postgres>,
) -> Result<(Vec<String>, Vec<String>), sqlx::Error> {
    let prefix_fut = sqlx::query_scalar!("SELECT word FROM prefix_word").fetch_all(pool);

    let suffix_fut = sqlx::query_scalar!("SELECT word FROM suffix_word").fetch_all(pool);

    let (prefix_result, suffix_result): (
        Result<Vec<String>, sqlx::Error>,
        Result<Vec<String>, sqlx::Error>,
    ) = tokio::join!(prefix_fut, suffix_fut);

    Ok((prefix_result?, suffix_result?))
}

#[derive(Debug, thiserror::Error)]
pub enum KeyVaultError {
    #[error("No more available words")]
    FullCapasity,

    #[error("Failed to load words: {0}")]
    Database(#[from] sqlx::Error),

    #[error("Word sets differ in length")]
    IncompatibleLength,

    #[error("Failed to get created at time: {0}")]
    TimeError(#[from] SystemTimeError),
}

#[derive(Debug)]
pub struct VaultValue {
    timestamp: u64,
    game_type: GameType,
}

pub struct KeyVault {
    word_count: u8,
    active_keys: Arc<DashMap<(String, String), VaultValue>>,
    prefix_words: Arc<Vec<String>>,
    suffix_words: Arc<Vec<String>>,
}

impl KeyVault {
    pub async fn load_words(pool: &Pool<Postgres>) -> Result<Self, KeyVaultError> {
        let (db_prefix, db_suffix) = get_word_sets(pool).await?;

        if db_prefix.len() != db_suffix.len() {
            return Err(KeyVaultError::IncompatibleLength);
        }

        let vault = Self {
            word_count: db_prefix.len() as u8,
            active_keys: Arc::new(DashMap::new()),
            prefix_words: Arc::new(db_prefix),
            suffix_words: Arc::new(db_suffix),
        };

        vault.spawn_vault_cleanup(pool);
        Ok(vault)
    }

    pub fn key_active(&self, key: &(String, String)) -> Option<GameType> {
        match self.active_keys.get(key) {
            Some(value) => Some(value.game_type),
            None => None,
        }
    }

    pub fn remove_key(&self, key: (String, String)) {
        self.active_keys.remove(&key);
    }

    fn random_idx(&self) -> Result<(usize, usize), KeyVaultError> {
        let mut rng = ChaCha8Rng::from_os_rng();
        let prefix_idx = rng.random_range(0..self.word_count as usize);
        let suffix_idx = rng.random_range(0..self.word_count as usize);

        Ok((prefix_idx, suffix_idx))
    }

    pub fn create_key(
        &self,
        _pool: &Pool<Postgres>,
        game_type: GameType,
    ) -> Result<String, KeyVaultError> {
        for _ in 0..100 {
            let Ok((idx1, idx2)) = self.random_idx() else {
                break; // Log outside loop
            };

            let key = (
                self.prefix_words[idx1].clone(),
                self.suffix_words[idx2].clone(),
            );

            if self.active_keys.contains_key(&key) {
                continue;
            }

            let timestamp = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
            let value = VaultValue {
                timestamp,
                game_type,
            };

            self.active_keys.insert(key.clone(), value);
            return Ok(format!("{} {}", key.0, key.1));
        }

        for i in 0..self.prefix_words.len() {
            for j in 0..self.suffix_words.len() {
                let key = (self.prefix_words[i].clone(), self.suffix_words[j].clone());

                if self.active_keys.contains_key(&key) {
                    continue;
                }

                let timestamp = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
                let value = VaultValue {
                    timestamp,
                    game_type,
                };

                self.active_keys.insert(key.clone(), value);
                return Ok(format!("{} {}", key.0, key.1));
            }
        }

        // Failed to find available key after exhaustive search
        Err(KeyVaultError::FullCapasity)
    }

    fn spawn_vault_cleanup(&self, _pool: &Pool<Postgres>) {
        let mut interval = tokio::time::interval(Duration::from_secs(3600));
        let active_keys = self.active_keys.clone();

        tokio::spawn(async move {
            loop {
                interval.tick().await;
                debug!("KeyVault is cleaning up its keys");

                let Ok(time) = SystemTime::now().duration_since(UNIX_EPOCH) else {
                    // Can't get system time - skip this cleanup cycle
                    continue;
                };

                let keys_before = active_keys.len();
                let timeout_threshold = time.as_secs() - 3600;

                active_keys.retain(|_, value| value.timestamp > timeout_threshold);

                let keys_after = active_keys.len();
                let removed_keys = keys_before - keys_after;

                if removed_keys > 0 {
                    warn!(
                        "Cleaned up {} expired game keys - indicates potential game crash or unexpected exit",
                        removed_keys
                    );
                }
            }
        });
    }
}
