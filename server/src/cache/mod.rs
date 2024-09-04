//! gRPC
//! provides Redis implementations for caching layer

#[macro_use]
pub mod macros;
pub mod pool;

use pool::RedisPool;
use serde::Deserialize;
use std::fmt::Debug;
use tonic::async_trait;

use tokio::time::{interval, Duration};

/// A consumer of Redis Queue data.
#[derive(Debug)]
pub struct Consumer {
    /// The Redis pool to use for consuming data
    pub pool: RedisPool,

    /// The time to sleep between consuming data
    pub sleep_ms: u64,
}

impl Consumer {
    /// Create a new Consumer
    pub async fn new(
        config: &crate::config::Config,
        key_folder: &str,
        sleep_ms: u64,
    ) -> Result<Self, ()> {
        RedisPool::new(config, key_folder)
            .await
            .map_err(|_| {
                cache_error!("could not get Redis pool for folder '{key_folder}'.");
            })
            .map(|pool| Self { pool, sleep_ms })
    }
}

/// Has a method to "process" items
#[async_trait]
pub trait Processor<T> {
    /// Process the items from the Redis queue and push to PostGis
    async fn process(&mut self, items: Vec<T>) -> Result<(), ()>;
}

/// A consumer of Redis Queue data.
#[async_trait]
pub trait IsConsumer<T>: Processor<T>
where
    T: for<'a> Deserialize<'a> + Clone + Debug + Send,
{
    /// The Redis pool to use for consuming data
    fn pool(&self) -> RedisPool;

    /// The time to sleep between consuming data
    fn sleep_ms(&self) -> u64;

    /// Starts a loop to consume data from the Redis queue
    #[cfg(not(tarpaulin_include))]
    // no_coverage: (Rnever) need running redis instance, not unit testable
    async fn begin(&mut self) -> Result<(), ()> {
        let mut redis_pool: RedisPool = self.pool();
        let mut connection = redis_pool.pool.get().await.map_err(|e| {
            cache_error!("could not get connection from Redis pool: {e}");
        })?;

        let mut interval = interval(Duration::from_millis(self.sleep_ms()));

        loop {
            let result = redis_pool.pop(&mut connection).await.map_err(|e| {
                cache_error!("(AircraftConsumer::begin) could not get aircraft from Redis: {e}");
            })?;

            let _ = self.process(result).await;
            interval.tick().await;
        }
    }
}

/// Implement the `IsConsumer` trait for `Consumer`
impl<T> IsConsumer<T> for Consumer
where
    Consumer: Processor<T>,
    T: for<'a> Deserialize<'a> + Clone + Debug + Send,
{
    fn pool(&self) -> RedisPool {
        self.pool.clone()
    }

    fn sleep_ms(&self) -> u64 {
        self.sleep_ms
    }
}
