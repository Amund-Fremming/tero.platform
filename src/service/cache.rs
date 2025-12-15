use std::{
    hash::{DefaultHasher, Hash, Hasher},
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};

use dashmap::DashMap;
use tokio::{task::JoinHandle, time};
use tracing::error;

use crate::models::error::ServerError;

// 20MB
pub static MAX_BYTE_SIZE: usize = 20_971_520;

fn generate_hash<T>(value: &T) -> u64
where
    T: Hash,
{
    let mut hasher = DefaultHasher::new();
    value.hash(&mut hasher);
    hasher.finish()
}

#[derive(Debug, Clone)]
pub struct CacheEntry<T: Clone + Sync + 'static> {
    pub(crate) timestamp: u64,
    pub(crate) value: T,
}

impl<T: Clone + Sync + 'static> CacheEntry<T> {
    pub(crate) fn new(value: T) -> Result<Self, ServerError> {
        Ok(Self {
            timestamp: SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs(),
            value,
        })
    }
}

#[derive(Debug)]
pub struct GustCache<T: Clone + Send + Sync + 'static> {
    cache: Arc<DashMap<u64, CacheEntry<T>>>,
    ttl: u64,
    cleanup_task: Option<JoinHandle<()>>,
    eviction_task: Option<JoinHandle<()>>,
}

impl<T: Clone + Send + Sync> GustCache<T> {
    pub fn from_ttl(ttl_secs: u64) -> Self {
        Self::setup(ttl_secs)
    }

    fn setup(ttl_secs: u64) -> Self {
        let mut cache = Self {
            cache: Arc::new(DashMap::new()),
            ttl: ttl_secs,
            cleanup_task: None,
            eviction_task: None,
        };

        cache.spawn_cleanup();
        cache.spawn_eviction();
        cache
    }

    pub async fn get_or<K, F>(&self, key: &K, on_failure: F) -> Result<T, ServerError>
    where
        F: AsyncFnOnce() -> Result<T, sqlx::Error>,
        K: Hash,
    {
        let key = generate_hash(key);

        if let Some(mut entry) = self.cache.get_mut(&key) {
            let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();

            if entry.timestamp + self.ttl > now {
                entry.timestamp = now;
                return Ok(entry.value.clone());
            }
        };

        let data = on_failure().await?;
        let cache_entry = CacheEntry::new(data.clone())?;
        self.cache.insert(key, cache_entry);

        Ok(data)
    }

    fn spawn_cleanup(&mut self) {
        let interval_seconds = (self.ttl / 2) + 1;
        let interval = time::Duration::from_secs(interval_seconds);

        let cache_pointer = self.cache.clone();
        let offset = self.ttl;

        let mut ticker = tokio::time::interval(interval);
        self.cleanup_task = Some(tokio::spawn(async move {
            loop {
                ticker.tick().await;
                let Ok(duration) = SystemTime::now().duration_since(UNIX_EPOCH) else {
                    error!("Failed to get secs from UNIX EPOCH");
                    continue;
                };

                let now = duration.as_secs();
                cache_pointer.retain(|_, value| now < value.timestamp + offset);
            }
        }));
    }

    fn spawn_eviction(&mut self) {
        let interval = time::Duration::from_secs(60 * 10);
        let mut ticker = tokio::time::interval(interval);
        let cache_pointer = self.cache.clone();

        self.eviction_task = Some(tokio::spawn(async move {
            loop {
                ticker.tick().await;

                let cache_byte_size: usize = cache_pointer
                    .iter()
                    .map(|entry| std::mem::size_of_val(&*entry))
                    .sum();

                if cache_byte_size < MAX_BYTE_SIZE {
                    continue;
                }

                let num_evictions = cache_pointer.len() * 70 / 100;
                let mut entries: Vec<(u64, u64)> = cache_pointer
                    .iter()
                    .map(|entry| (*entry.key(), entry.value().timestamp))
                    .collect();

                entries.sort_by_key(|(_, ts)| std::cmp::Reverse(*ts));
                let mut overflow: Vec<u64> = Vec::new();

                for _ in 0..num_evictions {
                    match entries.pop() {
                        None => break,
                        Some((key, _)) => overflow.push(key),
                    };
                }

                for key in overflow {
                    cache_pointer.remove(&key);
                }
            }
        }));
    }
}
