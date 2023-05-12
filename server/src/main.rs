//! Main function starting the server and initializing dependencies.

use log::info;
use svc_gis::config::Config;
use svc_gis::grpc;

/// Main entry point: starts gRPC Server on specified address and port
#[tokio::main]
#[cfg(not(tarpaulin_include))]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Will use default config settings if no environment vars are found.
    let config = match Config::from_env() {
        Ok(cfg) => cfg,
        Err(e) => {
            panic!("(main) could not parse config from environment: {}", e);
        }
    };

    let log_cfg: &str = config.log_config.as_str();
    if let Err(e) = log4rs::init_file(log_cfg, Default::default()) {
        panic!("(logger) could not parse {}. {}", log_cfg, e);
    }

    let _ = tokio::spawn(grpc::server::grpc_server(config)).await;

    info!("Server shutdown.");
    Ok(())
}
