//! Main function starting the server and initializing dependencies.

use log::info;
use svc_gis::config;
use svc_gis::grpc;
use svc_gis::postgis;

/// Main entry point: starts gRPC Server on specified address and port
#[tokio::main]
#[cfg(not(tarpaulin_include))]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Get environment variables
    let config = config::Config::from_env().unwrap_or_else(|e| {
        panic!("(main) could not parse config from environment: {}", e);
    });

    // Initialize logger
    let log_cfg: &str = config.log_config.as_str();
    log4rs::init_file(log_cfg, Default::default())
        .unwrap_or_else(|e| panic!("(logger) could not initialize logger. {}", e));

    // Create pool from PostgreSQL environment variables
    let pool = postgis::pool::create_pool(config.clone());

    // Create gRPC server
    let _ = tokio::spawn(grpc::server::grpc_server(config, pool.clone())).await;

    info!("Server shutdown.");
    Ok(())
}
