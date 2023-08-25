//! Main function starting the server and initializing dependencies.

use log::info;
use svc_gis::*;

/// Main entry point: starts gRPC Server on specified address and port
#[tokio::main]
#[cfg(not(tarpaulin_include))]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("(svc-gis) server startup.");

    // Will use default config settings if no environment vars are found.
    let config = Config::try_from_env().unwrap_or_default();

    init_logger(&config);

    // Create pool from PostgreSQL environment variables
    let pool = postgis::pool::create_pool(config.clone());

    // Create gRPC server
    let _ = tokio::spawn(grpc::server::grpc_server(config, None, pool.clone())).await;

    info!("Server shutdown.");
    Ok(())
}
