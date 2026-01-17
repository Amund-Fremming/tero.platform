use std::sync::Arc;
use std::time::Duration;

use moka::future::Cache;
use tracing::{debug, error};

use crate::models::{
    error::ServerError,
    game_base::{GameCacheKey, GameCategory, GameType},
};

// Approximate 20MB with ~10k entries
pub static MAX_CACHE_ENTRIES: u64 = 10_000;

#[derive(Debug, Clone)]
pub struct GustCache<T: Clone + Send + Sync + 'static> {
    cache: Arc<Cache<GameCacheKey, T>>,
}

impl<T: Clone + Send + Sync + 'static> GustCache<T> {
    pub fn from_ttl(ttl_secs: u64) -> Self {
        let cache = Cache::builder()
            .max_capacity(MAX_CACHE_ENTRIES)
            .time_to_idle(Duration::from_secs(ttl_secs))
            .build();

        Self {
            cache: Arc::new(cache),
        }
    }

    pub async fn get_or<F>(&self, key: GameCacheKey, on_failure: F) -> Result<T, ServerError>
    where
        F: Future<Output = Result<T, sqlx::Error>>,
    {
        match self
            .cache
            .try_get_with(key, async {
                on_failure.await.map_err(|e| ServerError::from(e))
            })
            .await
        {
            Ok(entry) => Ok(entry),
            Err(e) => {
                error!("Cache failed to get entry: {}", e);
                Err(ServerError::Internal(e.to_string()))
            }
        }
    }

    pub async fn invalidate_category(
        &self,
        game_type: GameType,
        category: &GameCategory,
    ) -> Result<(), ServerError> {
        debug!(
            "Invalidating cache for game_type={:?}, category={:?}",
            game_type, category
        );

        // Invalidate specific category and queries with no category filter
        let category = category.clone();
        match self.cache.invalidate_entries_if(move |key, _| {
            key.game_type == game_type
                && (key.category == Some(category.clone()) || key.category.is_none())
        }) {
            Ok(_) => Ok(()),
            Err(e) => {
                error!("Failed to invalidate category: {}", e);
                Err(ServerError::Internal(
                    "Cache error: Predicate error".to_string(),
                ))
            }
        }
    }
}
