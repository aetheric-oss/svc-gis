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
}

impl Config {
    /// Create a new `Config` object using environment variables
    pub fn from_env() -> Result<Self, ConfigError> {
        // read .env file if present
        dotenv().ok();

        config::Config::builder()
            .add_source(Environment::default().separator("__"))
            .build()?
            .try_deserialize()
    }
}
