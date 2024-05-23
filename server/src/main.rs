//! Main function starting the server and initializing dependencies.

use crate::types::{
    AircraftId, AircraftPosition, AircraftVelocity, REDIS_KEY_AIRCRAFT_ID,
    REDIS_KEY_AIRCRAFT_POSITION, REDIS_KEY_AIRCRAFT_VELOCITY,
};
use cache::Consumer;
use lib_common::logger::load_logger_config_from_file;
use log::info;
use svc_gis::cache::IsConsumer;
use svc_gis::*;

#[cfg(not(tarpaulin_include))]
// no_coverage: (Rnever) needs running backend, integration tests, these spin up threads
async fn start_redis_consumers(config: &Config) -> Result<(), ()> {
    //
    // Aircraft
    //
    let mut id_consumer = Consumer::new(config, REDIS_KEY_AIRCRAFT_ID, 500).await?;
    let mut position_consumer = Consumer::new(config, REDIS_KEY_AIRCRAFT_POSITION, 100).await?;
    let mut velocity_consumer = Consumer::new(config, REDIS_KEY_AIRCRAFT_VELOCITY, 100).await?;

    tokio::spawn(
        async move { <Consumer as IsConsumer<AircraftId>>::begin(&mut id_consumer).await },
    );

    tokio::spawn(async move {
        <Consumer as IsConsumer<AircraftPosition>>::begin(&mut position_consumer).await
    });

    tokio::spawn(async move {
        <Consumer as IsConsumer<AircraftVelocity>>::begin(&mut velocity_consumer).await
    });

    Ok(())
}

/// Main entry point: starts gRPC Server on specified address and port
#[tokio::main]
#[cfg(not(tarpaulin_include))]
// no_coverage: (Rnever) main entry point of the application
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Will use default config settings if no environment vars are found.
    let config = Config::try_from_env()
        .map_err(|e| format!("Failed to load configuration from environment: {}", e))?;

    // Try to load log configuration from the provided log file.
    // Will default to stdout debug logging if the file can not be loaded.
    load_logger_config_from_file(config.log_config.as_str())
        .await
        .or_else(|e| Ok::<(), String>(log::error!("(main) {}", e)))?;

    info!("(main) Server startup.");

    // Create pool from PostgreSQL environment variables
    let pool = postgis::pool::create_pool(config.clone()).map_err(|e| {
        let error = format!("Could not create pool: {:?}", e);
        log::error!("(main) {error}");
        error
    })?;

    crate::postgis::DEADPOOL_POSTGIS.set(pool).map_err(|e| {
        let error = format!("Could not set DEADPOOL_POSTGIS: {:?}", e);
        log::error!("(main) {error}");
        error
    })?;

    postgis::psql_init().await?;

    // Start the Redis consumers
    start_redis_consumers(&config).await.map_err(|_| {
        let error = "Could not start Redis consumers.";
        log::error!("(main) {error}");
        error
    })?;

    // Start GRPC Server
    tokio::spawn(grpc::server::grpc_server(config, None)).await?;

    info!("(main) Server shutdown.");

    // Make sure all log message are written/ displayed before shutdown
    log::logger().flush();

    Ok(())
}
