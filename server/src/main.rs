//! Main function starting the server and initializing dependencies.

use log::info;
use svc_gis::*;

/// Main entry point: starts gRPC Server on specified address and port
#[tokio::main]
#[cfg(not(tarpaulin_include))]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Will use default config settings if no environment vars are found.
    let config = Config::try_from_env().unwrap_or_default();

    // Try to load log configuration from the provided log file.
    // Will default to stdout debug logging if the file can not be loaded.
    load_logger_config_from_file(config.log_config.as_str())
        .await
        .or_else(|e| Ok::<(), String>(log::error!("(main) {}", e)))?;

    info!("(main) Server startup.");

    // Create pool from PostgreSQL environment variables
    let pool = postgis::pool::create_pool(config.clone());

    // Start GRPC Server
    tokio::spawn(grpc::server::grpc_server(config, None, pool.clone())).await?;

    info!("(main) Server shutdown.");

    // Make sure all log message are written/ displayed before shutdown
    log::logger().flush();

    Ok(())
}
