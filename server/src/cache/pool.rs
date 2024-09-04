//! Redis connection pool implementation

use deadpool_redis::{redis, Pool, Runtime};
use serde::Deserialize;
use std::fmt::{Debug, Display, Formatter, Result as FmtResult};
use std::num::NonZeroUsize;

/// Represents a pool of connections to a Redis server.
///
/// The [`RedisPool`] struct provides a managed pool of connections to a Redis server.
/// It allows clients to acquire and release connections from the pool and handles
/// connection management, such as connection pooling and reusing connections.
#[derive(Clone)]
pub struct RedisPool {
    /// The underlying pool of Redis connections.
    pub pool: Pool,
    /// The string prepended to the key being stored.
    key_folder: String,
}

impl Debug for RedisPool {
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("RedisPool")
            .field("key_folder", &self.key_folder)
            .finish()
    }
}

/// Represents errors that can occur during cache operations.
#[derive(Debug, Clone, Copy)]
pub enum CacheError {
    /// Could not build configuration for cache.
    CouldNotConfigure,

    /// Could not connect to the Redis pool.
    CouldNotConnect,

    /// The operation on the Redis cache failed.
    OperationFailed,
}

impl Display for CacheError {
    fn fmt(&self, f: &mut Formatter) -> FmtResult {
        match self {
            CacheError::CouldNotConfigure => write!(f, "Could not configure cache."),
            CacheError::CouldNotConnect => write!(f, "Could not connect to cache."),
            CacheError::OperationFailed => write!(f, "Cache operation failed."),
        }
    }
}

impl RedisPool {
    /// Create a new RedisPool
    /// The 'key_folder' argument is prepended to the key being stored. The
    ///  complete key will take the format \<folder\>:\<subset\>:\<subset\>:\<key\>.
    ///  This is used to differentiate keys inserted into Redis by different
    ///  microservices. For example, an ADS-B key in svc-telemetry might be
    ///  formatted `telemetry:adsb:1234567890`.
    pub async fn new(config: &crate::config::Config, key_folder: &str) -> Result<Self, ()> {
        // the .env file must have REDIS__URL="redis://\<host\>:\<port\>"
        let cfg: deadpool_redis::Config = config.redis.clone();
        let details = cfg.url.clone().ok_or_else(|| {
            cache_error!("no connection address found.");
        })?;

        cache_info!(
            "creating pool with key folder '{}' at {:?}...",
            key_folder,
            details
        );

        cfg.create_pool(Some(Runtime::Tokio1))
            .map_err(|e| {
                cache_error!("could not create pool: {}", e);
            })
            .map(|pool| {
                cache_info!("pool created.");
                Self {
                    pool,
                    key_folder: String::from(key_folder),
                }
            })
    }

    fn key_folder(&self) -> String {
        self.key_folder.clone()
    }

    fn process_bulk<T>(values: Vec<redis::Value>) -> Result<Vec<T>, CacheError>
    where
        T: for<'a> Deserialize<'a> + Clone + Debug,
    {
        // Remove nil values
        let values = values
            .into_iter()
            .filter_map(|value| match value {
                redis::Value::Nil => None,
                redis::Value::Bulk(values) => Some(values),
                _ => {
                    cache_error!("not valid data: {:?}", value);
                    None
                }
            })
            .flatten()
            .collect::<Vec<redis::Value>>();

        let values = values
            .into_iter()
            .filter_map(|value| {
                let redis::Value::Data(data) = value else {
                    cache_error!("not valid data: {:?}", value);
                    return None;
                };

                serde_json::from_slice::<T>(&data)
                    .map_err(|e| {
                        cache_error!("could not deserialize value: {:?}", e);
                    })
                    .map(|value| value.to_owned())
                    .ok()
            })
            .collect::<Vec<T>>();

        cache_debug!("retrieved values: {:?}", values);
        Ok(values)
    }

    ///
    /// Set the value of multiple keys
    ///
    #[cfg(not(tarpaulin_include))]
    // no_coverage: (Rnever) needs redis backend to integration test
    pub async fn pop<T, C>(&mut self, connection: &mut C) -> Result<Vec<T>, CacheError>
    where
        T: for<'a> Deserialize<'a> + Clone + Debug,
        C: redis::aio::ConnectionLike,
    {
        // TODO(R5): As static when that is supported
        let pop_count = NonZeroUsize::new(20).ok_or_else(|| {
            cache_error!("Operation failed, could not create NonZeroUsize.");
            CacheError::OperationFailed
        })?;

        let mut pipe = redis::pipe();
        let result = pipe
            .atomic()
            .rpop(self.key_folder(), Some(pop_count))
            .query_async(connection)
            .await
            .map_err(|e| {
                cache_error!("Operation failed, redis error: {}", e);
                CacheError::OperationFailed
            })?;

        let redis::Value::Bulk(values) = result else {
            cache_error!("Operation failed, unexpected redis response: {:?}", result);
            return Err(CacheError::OperationFailed);
        };

        if values.is_empty() {
            cache_debug!("No values found.");
            return Ok(vec![]);
        }

        RedisPool::process_bulk::<T>(values)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cache_error_display() {
        assert_eq!(
            format!("{}", CacheError::CouldNotConfigure),
            "Could not configure cache."
        );
        assert_eq!(
            format!("{}", CacheError::CouldNotConnect),
            "Could not connect to cache."
        );
        assert_eq!(
            format!("{}", CacheError::OperationFailed),
            "Cache operation failed."
        );
    }

    // #[tokio::test]
    // async fn test_redis_pool_debug() {
    //     let key_folder = "test";
    //     let pool = RedisPool::new(&crate::config::Config::default(), key_folder).await.unwrap();
    //     assert_eq!(
    //         format!("{:?}", pool),
    //         format!("RedisPool {{ key_folder: \"{key_folder}\" }}")
    //     );
    // }
}
