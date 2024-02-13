//! # Config
//!
//! Define and implement config options for module

use anyhow::Result;
use config::{ConfigError, Environment};
use dotenv::dotenv;
use serde::Deserialize;

/// struct holding configuration options
#[derive(Debug, Deserialize, Clone)]
pub struct Config {
    /// PostGIS configuration
    pub pg: deadpool_postgres::Config,
    /// path to CA certificate file
    pub db_ca_cert: String,
    /// path to client certificate file
    pub db_client_cert: String,
    /// path to client key file
    pub db_client_key: String,
    /// port to be used for gRPC server
    pub docker_port_grpc: u16,
    /// path to log configuration YAML file
    pub log_config: String,
    /// redis details
    pub redis: deadpool_redis::Config,
}

impl Default for Config {
    fn default() -> Self {
        log::warn!("(default) Creating Config object with default values.");
        Self::new()
    }
}

impl Config {
    /// Create new configuration object with default values
    pub fn new() -> Self {
        Config {
            docker_port_grpc: 50051,
            log_config: String::from("log4rs.yaml"),
            pg: deadpool_postgres::Config::new(),
            db_ca_cert: "".to_string(),
            db_client_cert: "".to_string(),
            db_client_key: "".to_string(),
            redis: deadpool_redis::Config {
                url: None,
                pool: None,
                connection: None,
            },
        }
    }

    /// Create a new `Config` object using environment variables
    pub fn try_from_env() -> Result<Self, ConfigError> {
        // read .env file if present
        dotenv().ok();
        let default_config = Config::default();

        config::Config::builder()
            .set_default("docker_port_grpc", default_config.docker_port_grpc)?
            .set_default("log_config", default_config.log_config)?
            .add_source(Environment::default().separator("__"))
            .build()?
            .try_deserialize()
    }
}

#[cfg(test)]
mod tests {
    use super::Config;

    #[tokio::test]
    async fn test_config_from_default() {
        crate::get_log_handle().await;
        ut_info!("(test_config_from_default) Start.");

        let config = Config::default();

        assert_eq!(config.docker_port_grpc, 50051);
        assert_eq!(config.log_config, String::from("log4rs.yaml"));
        assert!(config.redis.url.is_none());
        assert!(config.redis.pool.is_none());
        assert!(config.redis.connection.is_none());

        ut_info!("(test_config_from_default) Success.");
    }

    #[tokio::test]
    async fn test_config_from_env() {
        crate::get_log_handle().await;
        ut_info!("(test_config_from_default) Start.");

        std::env::set_var("DOCKER_PORT_GRPC", "6789");
        std::env::set_var("LOG_CONFIG", "config_file.yaml");
        std::env::set_var("REDIS__URL", "redis://test_redis:6379");
        std::env::set_var("REDIS__POOL__MAX_SIZE", "16");
        std::env::set_var("REDIS__POOL__TIMEOUTS__WAIT__SECS", "2");
        std::env::set_var("REDIS__POOL__TIMEOUTS__WAIT__NANOS", "0");

        let config = Config::try_from_env();
        assert!(config.is_ok());
        let config = config.unwrap();

        assert_eq!(config.docker_port_grpc, 6789);
        assert_eq!(config.log_config, String::from("config_file.yaml"));
        assert_eq!(
            config.redis.url,
            Some(String::from("redis://test_redis:6379"))
        );
        assert!(config.redis.pool.is_some());

        ut_info!("(test_config_from_env) Success.");
    }
}
