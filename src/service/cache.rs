use std::sync::Arc;
use std::time::Duration;

use moka::future::Cache;
use tracing::{debug, error};

use crate::models::{
    error::ServerError,
    game::{GameCacheKey, GameCategory, GameType},
};

/// INFO:
///     I accepted that the eviction on max cache size will make the cache inconsistent.
///     A game might not appear or appear twice, but will be conistent after the ttl.
///      Approximate 20MB with ~10k entries
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
            .support_invalidation_closures()
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
            .try_get_with(key, async { on_failure.await.map_err(ServerError::from) })
            .await
        {
            Ok(entry) => Ok(entry),
            Err(e) => {
                error!("Cache failed to get entry: {}", e);
                Err(ServerError::Internal(e.to_string()))
            }
        }
    }

    pub async fn invalidate(
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use tokio::time::sleep;

    fn make_key(game_type: GameType, category: Option<GameCategory>, page: u16) -> GameCacheKey {
        GameCacheKey {
            game_type,
            category,
            page_num: page,
        }
    }

    #[tokio::test]
    async fn test_get_or_cache_hit() {
        let cache: GustCache<String> = GustCache::from_ttl(60);
        let key = make_key(GameType::Quiz, Some(GameCategory::Vors), 0);

        let call_count = Arc::new(AtomicUsize::new(0));
        let call_count_clone = call_count.clone();

        // First call - should invoke the fallback
        let result = cache
            .get_or(key.clone(), async move {
                call_count_clone.fetch_add(1, Ordering::SeqCst);
                Ok::<_, sqlx::Error>("first_value".to_string())
            })
            .await
            .unwrap();

        assert_eq!(result, "first_value");
        assert_eq!(call_count.load(Ordering::SeqCst), 1);

        // Second call - should return cached value, NOT invoke fallback
        let call_count_clone2 = call_count.clone();
        let result = cache
            .get_or(key.clone(), async move {
                call_count_clone2.fetch_add(1, Ordering::SeqCst);
                Ok::<_, sqlx::Error>("second_value".to_string())
            })
            .await
            .unwrap();

        assert_eq!(result, "first_value"); // Still returns cached value
        assert_eq!(call_count.load(Ordering::SeqCst), 1); // Fallback NOT called again
    }

    #[tokio::test]
    async fn test_get_or_different_keys_are_separate() {
        let cache: GustCache<String> = GustCache::from_ttl(60);

        let key_quiz = make_key(GameType::Quiz, Some(GameCategory::Vors), 0);
        let key_duel = make_key(GameType::Duel, Some(GameCategory::Vors), 0);

        let _ = cache
            .get_or(key_quiz.clone(), async { Ok("quiz_value".to_string()) })
            .await
            .unwrap();

        let _ = cache
            .get_or(key_duel.clone(), async { Ok("duel_value".to_string()) })
            .await
            .unwrap();

        // Fetch again - should return cached values
        let quiz_result = cache
            .get_or(key_quiz, async { Ok("new_quiz".to_string()) })
            .await
            .unwrap();

        let duel_result = cache
            .get_or(key_duel, async { Ok("new_duel".to_string()) })
            .await
            .unwrap();

        assert_eq!(quiz_result, "quiz_value");
        assert_eq!(duel_result, "duel_value");
    }

    #[tokio::test]
    async fn test_invalidate_removes_matching_category() {
        let cache: GustCache<String> = GustCache::from_ttl(60);

        // Same game type, different categories
        let key_quiz_vors = make_key(GameType::Quiz, Some(GameCategory::Vors), 0);
        let key_quiz_boys = make_key(GameType::Quiz, Some(GameCategory::Boys), 0);
        // Different game type, same category (should NOT be invalidated)
        let key_duel_vors = make_key(GameType::Duel, Some(GameCategory::Vors), 0);

        // Populate cache
        let _ = cache
            .get_or(key_quiz_vors.clone(), async { Ok("quiz_vors".to_string()) })
            .await;
        let _ = cache
            .get_or(key_quiz_boys.clone(), async { Ok("quiz_boys".to_string()) })
            .await;
        let _ = cache
            .get_or(key_duel_vors.clone(), async { Ok("duel_vors".to_string()) })
            .await;

        // Invalidate only Quiz + Vors
        cache
            .invalidate(GameType::Quiz, &GameCategory::Vors)
            .await
            .unwrap();

        // Quiz Vors should be invalidated
        let quiz_vors_result = cache
            .get_or(key_quiz_vors, async { Ok("new_quiz_vors".to_string()) })
            .await
            .unwrap();

        // Quiz Boys should still be cached (different category)
        let quiz_boys_result = cache
            .get_or(key_quiz_boys, async { Ok("new_quiz_boys".to_string()) })
            .await
            .unwrap();

        // Duel Vors should still be cached (different game type, same category)
        let duel_vors_result = cache
            .get_or(key_duel_vors, async { Ok("new_duel_vors".to_string()) })
            .await
            .unwrap();

        assert_eq!(quiz_vors_result, "new_quiz_vors"); // Was invalidated
        assert_eq!(quiz_boys_result, "quiz_boys"); // Still cached
        assert_eq!(duel_vors_result, "duel_vors"); // Still cached (different game type)
    }

    #[tokio::test]
    async fn test_invalidate_also_removes_none_category() {
        let cache: GustCache<String> = GustCache::from_ttl(60);

        let key_with_category = make_key(GameType::Quiz, Some(GameCategory::Vors), 0);
        let key_no_category = make_key(GameType::Quiz, None, 0);

        // Populate cache
        let _ = cache
            .get_or(key_with_category.clone(), async {
                Ok("with_cat".to_string())
            })
            .await;
        let _ = cache
            .get_or(key_no_category.clone(), async { Ok("no_cat".to_string()) })
            .await;

        // Invalidate Vors - should also invalidate None category
        cache
            .invalidate(GameType::Quiz, &GameCategory::Vors)
            .await
            .unwrap();

        // Both should be invalidated
        let with_cat_result = cache
            .get_or(key_with_category, async { Ok("new_with".to_string()) })
            .await
            .unwrap();

        let no_cat_result = cache
            .get_or(key_no_category, async { Ok("new_no".to_string()) })
            .await
            .unwrap();

        assert_eq!(with_cat_result, "new_with");
        assert_eq!(no_cat_result, "new_no");
    }

    #[tokio::test]
    async fn test_invalidate_does_not_affect_other_game_types() {
        let cache: GustCache<String> = GustCache::from_ttl(60);

        let key_quiz = make_key(GameType::Quiz, Some(GameCategory::Vors), 0);
        let key_duel = make_key(GameType::Duel, Some(GameCategory::Vors), 0);

        // Populate cache
        let _ = cache
            .get_or(key_quiz.clone(), async { Ok("quiz".to_string()) })
            .await;
        let _ = cache
            .get_or(key_duel.clone(), async { Ok("duel".to_string()) })
            .await;

        // Invalidate Quiz Vors
        cache
            .invalidate(GameType::Quiz, &GameCategory::Vors)
            .await
            .unwrap();

        // Quiz should be invalidated
        let quiz_result = cache
            .get_or(key_quiz, async { Ok("new_quiz".to_string()) })
            .await
            .unwrap();

        // Duel should still be cached
        let duel_result = cache
            .get_or(key_duel, async { Ok("new_duel".to_string()) })
            .await
            .unwrap();

        assert_eq!(quiz_result, "new_quiz");
        assert_eq!(duel_result, "duel"); // Still cached
    }

    #[tokio::test]
    async fn test_ttl_expiration() {
        let cache: GustCache<String> = GustCache::from_ttl(1); // 1 second TTL

        let key = make_key(GameType::Quiz, Some(GameCategory::Vors), 0);

        // Populate cache
        let _ = cache
            .get_or(key.clone(), async { Ok("initial".to_string()) })
            .await;

        // Immediately check - should be cached
        let result = cache
            .get_or(key.clone(), async { Ok("new_value".to_string()) })
            .await
            .unwrap();
        assert_eq!(result, "initial");

        // Wait for TTL to expire
        sleep(Duration::from_secs(2)).await;

        // Force moka to run its internal maintenance
        cache.cache.run_pending_tasks().await;

        // Now should fetch new value
        let result = cache
            .get_or(key, async { Ok("after_ttl".to_string()) })
            .await
            .unwrap();
        assert_eq!(result, "after_ttl");
    }

    #[tokio::test]
    async fn test_different_pages_are_separate_entries() {
        let cache: GustCache<String> = GustCache::from_ttl(60);

        let key_page_0 = make_key(GameType::Quiz, Some(GameCategory::Vors), 0);
        let key_page_1 = make_key(GameType::Quiz, Some(GameCategory::Vors), 1);

        let _ = cache
            .get_or(key_page_0.clone(), async { Ok("page_0".to_string()) })
            .await;
        let _ = cache
            .get_or(key_page_1.clone(), async { Ok("page_1".to_string()) })
            .await;

        let page_0_result = cache
            .get_or(key_page_0, async { Ok("new_page_0".to_string()) })
            .await
            .unwrap();

        let page_1_result = cache
            .get_or(key_page_1, async { Ok("new_page_1".to_string()) })
            .await
            .unwrap();

        assert_eq!(page_0_result, "page_0");
        assert_eq!(page_1_result, "page_1");
    }

    #[tokio::test]
    async fn test_invalidate_removes_all_pages_for_category() {
        let cache: GustCache<String> = GustCache::from_ttl(60);

        let key_page_0 = make_key(GameType::Quiz, Some(GameCategory::Vors), 0);
        let key_page_1 = make_key(GameType::Quiz, Some(GameCategory::Vors), 1);
        let key_page_2 = make_key(GameType::Quiz, Some(GameCategory::Vors), 2);

        // Populate all pages
        let _ = cache
            .get_or(key_page_0.clone(), async { Ok("p0".to_string()) })
            .await;
        let _ = cache
            .get_or(key_page_1.clone(), async { Ok("p1".to_string()) })
            .await;
        let _ = cache
            .get_or(key_page_2.clone(), async { Ok("p2".to_string()) })
            .await;

        // Invalidate category
        cache
            .invalidate(GameType::Quiz, &GameCategory::Vors)
            .await
            .unwrap();

        // All pages should be invalidated
        let r0 = cache
            .get_or(key_page_0, async { Ok("new_p0".to_string()) })
            .await
            .unwrap();
        let r1 = cache
            .get_or(key_page_1, async { Ok("new_p1".to_string()) })
            .await
            .unwrap();
        let r2 = cache
            .get_or(key_page_2, async { Ok("new_p2".to_string()) })
            .await
            .unwrap();

        assert_eq!(r0, "new_p0");
        assert_eq!(r1, "new_p1");
        assert_eq!(r2, "new_p2");
    }
}
