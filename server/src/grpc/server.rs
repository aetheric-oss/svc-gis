//! gRPC server implementation
/// module generated from proto/svc-template-rust-grpc.proto

pub mod grpc_server {
    #![allow(unused_qualifications, missing_docs)]
    tonic::include_proto!("grpc");
}

use crate::postgis::aircraft::update_aircraft_position;
use crate::postgis::nofly::update_nofly;
use crate::postgis::routing::{best_path, PathType};
use crate::postgis::vertiport::update_vertiports;
use crate::postgis::waypoint::update_waypoints;
pub use grpc_server::rpc_service_server::{RpcService, RpcServiceServer};
pub use grpc_server::NodeType;
use grpc_server::{ReadyRequest, ReadyResponse};

use crate::shutdown_signal;
use std::fmt::Debug;
use std::net::SocketAddr;
use tonic::transport::Server;
use tonic::{Request, Response, Status};

/// struct to implement the gRPC server functions
#[derive(Debug, Clone)]
pub struct ServerImpl {
    /// Postgres connection pool
    pub pool: deadpool_postgres::Pool,
}

#[cfg(not(feature = "stub_server"))]
#[tonic::async_trait]
impl RpcService for ServerImpl {
    /// Returns ready:true when service is available
    #[cfg(not(tarpaulin_include))]
    async fn is_ready(
        &self,
        _request: Request<ReadyRequest>,
    ) -> Result<Response<ReadyResponse>, Status> {
        grpc_debug!("(grpc is_ready) entry.");
        let response = ReadyResponse { ready: true };
        Ok(Response::new(response))
    }

    #[cfg(not(tarpaulin_include))]
    async fn update_vertiports(
        &self,
        request: Request<grpc_server::UpdateVertiportsRequest>,
    ) -> Result<Response<grpc_server::UpdateResponse>, Status> {
        grpc_debug!("(grpc update_vertiports) entry.");

        // Update nodes in PostGIS
        match update_vertiports(request.into_inner().vertiports, self.pool.clone()).await {
            Ok(_) => Ok(Response::new(grpc_server::UpdateResponse { updated: true })),
            Err(e) => {
                grpc_error!("(grpc update_vertiports) error updating vertiports.");
                Err(Status::internal(e.to_string()))
            }
        }
    }

    #[cfg(not(tarpaulin_include))]
    async fn update_waypoints(
        &self,
        request: Request<grpc_server::UpdateWaypointsRequest>,
    ) -> Result<Response<grpc_server::UpdateResponse>, Status> {
        grpc_debug!("(grpc update_waypoints) entry.");

        // Update nodes in PostGIS
        match update_waypoints(request.into_inner().waypoints, self.pool.clone()).await {
            Ok(_) => Ok(Response::new(grpc_server::UpdateResponse { updated: true })),
            Err(e) => {
                grpc_error!("(grpc update_waypoints) error updating nodes.");
                Err(Status::internal(e.to_string()))
            }
        }
    }

    #[cfg(not(tarpaulin_include))]
    async fn update_no_fly_zones(
        &self,
        request: Request<grpc_server::UpdateNoFlyZonesRequest>,
    ) -> Result<Response<grpc_server::UpdateResponse>, Status> {
        grpc_debug!("(grpc update_no_fly_zones) entry.");

        // Update nodes in PostGIS
        match update_nofly(request.into_inner().zones, self.pool.clone()).await {
            Ok(_) => Ok(Response::new(grpc_server::UpdateResponse { updated: true })),
            Err(e) => {
                grpc_error!("(grpc update_no_fly_zones) error updating zones.");
                Err(Status::invalid_argument(e.to_string()))
            }
        }
    }

    #[cfg(not(tarpaulin_include))]
    async fn update_aircraft_position(
        &self,
        request: Request<grpc_server::UpdateAircraftPositionRequest>,
    ) -> Result<Response<grpc_server::UpdateResponse>, Status> {
        grpc_debug!("(grpc update_aircraft) entry.");
        // Update aircraft in PostGIS
        match update_aircraft_position(request.into_inner().aircraft, self.pool.clone()).await {
            Ok(_) => Ok(Response::new(grpc_server::UpdateResponse { updated: true })),
            Err(e) => {
                grpc_error!("(grpc update_aircraft) error updating aircraft.");
                Err(Status::internal(e.to_string()))
            }
        }
    }

    #[cfg(not(tarpaulin_include))]
    async fn best_path(
        &self,
        request: Request<grpc_server::BestPathRequest>,
    ) -> Result<Response<grpc_server::BestPathResponse>, Status> {
        grpc_debug!("(grpc best_path) entry.");
        let request = request.into_inner();

        let path_type = match num::FromPrimitive::from_i32(request.start_type) {
            Some(NodeType::Vertiport) => PathType::PortToPort,
            Some(NodeType::Aircraft) => PathType::AircraftToPort,
            _ => {
                grpc_error!("(grpc best_path) invalid start node type.");
                return Err(Status::invalid_argument(
                    "Invalid start node type. Must be vertiport or aircraft.",
                ));
            }
        };

        match best_path(path_type, request, self.pool.clone()).await {
            Ok(segments) => {
                let response = grpc_server::BestPathResponse { segments };
                Ok(Response::new(response))
            }
            Err(e) => {
                grpc_error!("(grpc best_path) error getting best path.");
                Err(Status::internal(e.to_string()))
            }
        }
    }
}

/// Starts the grpc servers for this microservice using the provided configuration
///
/// # Example:
/// ```
/// use svc_gis::grpc::server::grpc_server;
/// use svc_gis::config::Config;
/// use deadpool_postgres::{tokio_postgres::NoTls, Runtime};
/// async fn example() -> Result<(), tokio::task::JoinError> {
///     let config = Config::from_env().unwrap();
///     let pool = config.pg.create_pool(Some(Runtime::Tokio1), NoTls).unwrap();
///     tokio::spawn(grpc_server(config, None, pool)).await
/// }
/// ```
#[cfg(not(tarpaulin_include))]
pub async fn grpc_server(
    config: crate::config::Config,
    shutdown_rx: Option<tokio::sync::oneshot::Receiver<()>>,
    pool: deadpool_postgres::Pool,
) {
    grpc_debug!("(grpc_server) entry.");

    // Grpc Server
    let grpc_port = config.docker_port_grpc;
    let full_grpc_addr: SocketAddr = match format!("[::]:{}", grpc_port).parse() {
        Ok(addr) => addr,
        Err(e) => {
            grpc_error!("(grpc_server) Failed to parse gRPC address: {}", e);
            return;
        }
    };

    let imp = ServerImpl { pool };
    let (mut health_reporter, health_service) = tonic_health::server::health_reporter();
    health_reporter
        .set_serving::<RpcServiceServer<ServerImpl>>()
        .await;

    //start server
    grpc_info!(
        "(grpc_server) Starting gRPC services on: {}.",
        full_grpc_addr
    );
    match Server::builder()
        .add_service(health_service)
        .add_service(RpcServiceServer::new(imp))
        .serve_with_shutdown(full_grpc_addr, shutdown_signal("grpc", shutdown_rx))
        .await
    {
        Ok(_) => grpc_info!("(grpc_server) gRPC server running at: {}.", full_grpc_addr),
        Err(e) => {
            grpc_error!("(grpc_server) Could not start gRPC server: {}", e);
        }
    };
}

#[cfg(feature = "stub_server")]
#[tonic::async_trait]
impl RpcService for ServerImpl {
    #[cfg(not(tarpaulin_include))]
    async fn is_ready(
        &self,
        _request: Request<ReadyRequest>,
    ) -> Result<Response<ReadyResponse>, Status> {
        grpc_warn!("(grpc is_ready MOCK) entry.");
        let response = ReadyResponse { ready: true };
        Ok(Response::new(response))
    }

    #[cfg(not(tarpaulin_include))]
    async fn update_vertiports(
        &self,
        request: Request<grpc_server::UpdateVertiportsRequest>,
    ) -> Result<Response<grpc_server::UpdateResponse>, Status> {
        grpc_warn!("(grpc update_vertiports MOCK) entry.");

        // Update nodes in PostGIS
        match update_vertiports(request.into_inner().vertiports, self.pool.clone()).await {
            Ok(_) => Ok(Response::new(grpc_server::UpdateResponse { updated: true })),
            Err(e) => {
                grpc_error!("(grpc update_vertiports MOCK) error updating vertiports.");
                Err(Status::internal(e.to_string()))
            }
        }
    }

    #[cfg(not(tarpaulin_include))]
    async fn update_waypoints(
        &self,
        request: Request<grpc_server::UpdateWaypointsRequest>,
    ) -> Result<Response<grpc_server::UpdateResponse>, Status> {
        grpc_warn!("(grpc update_waypoints MOCK) entry.");

        // Update nodes in PostGIS
        match update_waypoints(request.into_inner().waypoints, self.pool.clone()).await {
            Ok(_) => Ok(Response::new(grpc_server::UpdateResponse { updated: true })),
            Err(e) => {
                grpc_error!("(grpc update_waypoints MOCK) error updating nodes.");
                Err(Status::internal(e.to_string()))
            }
        }
    }

    #[cfg(not(tarpaulin_include))]
    async fn update_no_fly_zones(
        &self,
        request: Request<grpc_server::UpdateNoFlyZonesRequest>,
    ) -> Result<Response<grpc_server::UpdateResponse>, Status> {
        grpc_warn!("(grpc update_no_fly_zones MOCK) entry.");

        // Update nodes in PostGIS
        match update_nofly(request.into_inner().zones, self.pool.clone()).await {
            Ok(_) => Ok(Response::new(grpc_server::UpdateResponse { updated: true })),
            Err(e) => {
                grpc_error!("(grpc update_no_fly_zones MOCK) error updating zones.");
                Err(Status::invalid_argument(e.to_string()))
            }
        }
    }

    #[cfg(not(tarpaulin_include))]
    async fn update_aircraft_position(
        &self,
        request: Request<grpc_server::UpdateAircraftPositionRequest>,
    ) -> Result<Response<grpc_server::UpdateResponse>, Status> {
        grpc_warn!("(grpc update_aircraft MOCK) entry.");
        // Update aircraft in PostGIS
        match update_aircraft_position(request.into_inner().aircraft, self.pool.clone()).await {
            Ok(_) => Ok(Response::new(grpc_server::UpdateResponse { updated: true })),
            Err(e) => {
                grpc_error!("(grpc update_aircraft MOCK) error updating aircraft.");
                Err(Status::internal(e.to_string()))
            }
        }
    }

    #[cfg(not(tarpaulin_include))]
    async fn best_path(
        &self,
        request: Request<grpc_server::BestPathRequest>,
    ) -> Result<Response<grpc_server::BestPathResponse>, Status> {
        grpc_warn!("(grpc best_path MOCK) entry.");
        let request = request.into_inner();

        let path_type = match num::FromPrimitive::from_i32(request.start_type) {
            Some(NodeType::Vertiport) => PathType::PortToPort,
            Some(NodeType::Aircraft) => PathType::AircraftToPort,
            _ => {
                grpc_error!("(grpc best_path MOCK) invalid start node type.");
                return Err(Status::invalid_argument(
                    "Invalid start node type. Must be vertiport or aircraft.",
                ));
            }
        };

        match best_path(path_type, request, self.pool.clone()).await {
            Ok(segments) => {
                let response = grpc_server::BestPathResponse { segments };
                Ok(Response::new(response))
            }
            Err(e) => {
                grpc_error!("(grpc best_path MOCK) error getting best path.");
                Err(Status::internal(e.to_string()))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use deadpool_postgres::{Config, ManagerConfig, RecyclingMethod, Runtime};
    use tokio_postgres::NoTls;

    fn get_pool() -> deadpool_postgres::Pool {
        let mut cfg = Config::new();
        cfg.dbname = Some("deadpool".to_string());
        cfg.manager = Some(ManagerConfig {
            recycling_method: RecyclingMethod::Fast,
        });
        cfg.create_pool(Some(Runtime::Tokio1), NoTls).unwrap()
    }

    #[tokio::test]
    async fn test_grpc_server_is_ready() {
        let imp = ServerImpl { pool: get_pool() };
        let result = imp.is_ready(Request::new(ReadyRequest {})).await;
        assert!(result.is_ok());
        let result: ReadyResponse = result.unwrap().into_inner();
        assert_eq!(result.ready, true);
    }
}
