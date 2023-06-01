//! gRPC server implementation

///module generated from proto/svc-svc-gis-grpc.proto
pub mod grpc_server {
    #![allow(unused_qualifications, missing_docs)]
    tonic::include_proto!("grpc");
}
use crate::postgis::node::{nodes_grpc_to_gis, update_nodes};
use crate::postgis::nofly::{nofly_grpc_to_gis, update_nofly};
use grpc_server::rpc_service_server::{RpcService, RpcServiceServer};
use grpc_server::{ReadyRequest, ReadyResponse};

use crate::config::Config;
use crate::shutdown_signal;

use std::fmt::Debug;
use std::net::SocketAddr;
use tonic::transport::Server;
use tonic::{Request, Response, Status};

/// struct to implement the gRPC server functions
#[derive(Debug, Clone)]
pub struct GRPCServerImpl {
    pool: deadpool_postgres::Pool,
}

#[tonic::async_trait]
impl RpcService for GRPCServerImpl {
    /// Returns ready:true when service is available
    async fn is_ready(
        &self,
        _request: Request<ReadyRequest>,
    ) -> Result<Response<ReadyResponse>, Status> {
        grpc_debug!("(grpc is_ready) entry.");
        let response = ReadyResponse { ready: true };
        Ok(Response::new(response))
    }

    async fn update_nodes(
        &self,
        request: Request<grpc_server::UpdateNodesRequest>,
    ) -> Result<Response<grpc_server::UpdateNodesResponse>, Status> {
        grpc_debug!("(grpc update_node) entry.");

        // Sanitize inputs
        let nodes = match nodes_grpc_to_gis(request.into_inner().nodes) {
            Ok(nodes) => nodes,
            Err(e) => return Err(Status::invalid_argument(e.to_string())),
        };

        // Update nodes in PostGIS
        match update_nodes(nodes, self.pool.clone()).await {
            Ok(_) => Ok(Response::new(grpc_server::UpdateNodesResponse {
                updated: true,
            })),
            Err(_) => {
                grpc_error!("(grpc update_node) error updating nodes.");
                Err(Status::internal("Error updating nodes."))
            }
        }
    }

    async fn update_no_fly_zones(
        &self,
        request: Request<grpc_server::UpdateNoFlyZonesRequest>,
    ) -> Result<Response<grpc_server::UpdateNoFlyZonesResponse>, Status> {
        grpc_debug!("(grpc update_no_fly_zones) entry.");

        // Sanitize inputs
        let zones = match nofly_grpc_to_gis(request.into_inner().zones) {
            Ok(zones) => zones,
            Err(e) => return Err(Status::invalid_argument(e.to_string())),
        };

        // Update nodes in PostGIS
        match update_nofly(zones, self.pool.clone()).await {
            Ok(_) => Ok(Response::new(grpc_server::UpdateNoFlyZonesResponse {
                updated: true,
            })),
            Err(_) => {
                grpc_error!("(grpc update_no_fly_zones) error updating zones.");
                Err(Status::internal("Error updating zones."))
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
///     tokio::spawn(grpc_server(config, pool)).await
/// }
/// ```
#[cfg(not(tarpaulin_include))]
pub async fn grpc_server(config: Config, pool: deadpool_postgres::Pool) {
    grpc_debug!("(grpc_server) entry.");

    // GRPC Server
    let grpc_port = config.docker_port_grpc;
    let full_grpc_addr: SocketAddr = match format!("[::]:{}", grpc_port).parse() {
        Ok(addr) => addr,
        Err(e) => {
            grpc_error!("Failed to parse gRPC address: {}", e);
            return;
        }
    };

    let (mut health_reporter, health_service) = tonic_health::server::health_reporter();
    let imp = GRPCServerImpl { pool };

    health_reporter
        .set_serving::<RpcServiceServer<GRPCServerImpl>>()
        .await;

    //start server
    grpc_info!("Starting GRPC servers on: {}.", full_grpc_addr);
    match Server::builder()
        .add_service(health_service)
        .add_service(RpcServiceServer::new(imp))
        .serve_with_shutdown(full_grpc_addr, shutdown_signal("grpc"))
        .await
    {
        Ok(_) => grpc_info!("gRPC server running at: {}.", full_grpc_addr),
        Err(e) => {
            grpc_error!("could not start gRPC server: {}", e);
        }
    };
}
