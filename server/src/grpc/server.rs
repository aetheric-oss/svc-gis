//! gRPC server implementation
/// module generated from proto/svc-template-rust-grpc.proto

pub mod grpc_server {
    #![allow(unused_qualifications, missing_docs)]
    tonic::include_proto!("grpc");
}

use crate::postgis::utils::distance_meters;
use crate::postgis::{best_path::PathError, *};
use crate::shutdown_signal;
pub use grpc_server::rpc_service_server::{RpcService, RpcServiceServer};
use grpc_server::{ReadyRequest, ReadyResponse};
use lib_common::time::{DateTime, Utc};
use postgis::ewkb::PointZ;
use std::fmt::Debug;
use std::net::SocketAddr;
use tonic::transport::Server;
use tonic::{Request, Response, Status};

/// struct to implement the gRPC server functions
#[derive(Debug, Copy, Clone)]
pub struct ServerImpl {}

#[cfg(not(feature = "stub_server"))]
#[tonic::async_trait]
impl RpcService for ServerImpl {
    /// Returns ready:true when service is available
    async fn is_ready(
        &self,
        _request: Request<ReadyRequest>,
    ) -> Result<Response<ReadyResponse>, Status> {
        grpc_debug!("entry.");
        let response = ReadyResponse { ready: true };
        Ok(Response::new(response))
    }

    async fn update_vertiports(
        &self,
        request: Request<grpc_server::UpdateVertiportsRequest>,
    ) -> Result<Response<grpc_server::UpdateResponse>, Status> {
        grpc_debug!("entry.");

        // Update nodes in PostGIS
        let vertiports = request.into_inner().vertiports;
        vertiport::update_vertiports(vertiports)
            .await
            .map_err(|e| {
                grpc_error!("error updating vertiports: {}", e);
                Status::internal(e.to_string())
            })?;

        Ok(Response::new(grpc_server::UpdateResponse { updated: true }))
    }

    async fn update_waypoints(
        &self,
        request: Request<grpc_server::UpdateWaypointsRequest>,
    ) -> Result<Response<grpc_server::UpdateResponse>, Status> {
        grpc_debug!("entry.");

        // Update nodes in PostGIS
        let waypoints = request.into_inner().waypoints;
        waypoint::update_waypoints(waypoints).await.map_err(|e| {
            grpc_error!("error updating nodes: {}", e);
            Status::internal(e.to_string())
        })?;

        Ok(Response::new(grpc_server::UpdateResponse { updated: true }))
    }

    async fn update_zones(
        &self,
        request: Request<grpc_server::UpdateZonesRequest>,
    ) -> Result<Response<grpc_server::UpdateResponse>, Status> {
        grpc_debug!("entry.");

        // Update nodes in PostGIS
        let zones = request.into_inner().zones;
        zone::update_zones(zones).await.map_err(|e| {
            grpc_error!("error updating zones: {}", e);
            Status::internal(e.to_string())
        })?;

        Ok(Response::new(grpc_server::UpdateResponse { updated: true }))
    }

    async fn update_flight_path(
        &self,
        request: Request<grpc_server::UpdateFlightPathRequest>,
    ) -> Result<Response<grpc_server::UpdateResponse>, Status> {
        grpc_debug!("entry.");

        // Update nodes in PostGIS
        let request = request.into_inner();
        flight::update_flight_path(request).await.map_err(|e| {
            grpc_error!("error updating flight path: {}", e);
            Status::internal(e.to_string())
        })?;

        Ok(Response::new(grpc_server::UpdateResponse { updated: true }))
    }

    async fn best_path(
        &self,
        request: Request<grpc_server::BestPathRequest>,
    ) -> Result<Response<grpc_server::BestPathResponse>, Status> {
        grpc_debug!("entry.");
        let request = request.into_inner();

        let paths = best_path::best_path(request).await.map_err(|e| {
            grpc_error!("error getting best path.");
            Status::internal(e.to_string())
        })?;

        Ok(Response::new(grpc_server::BestPathResponse { paths }))
    }

    async fn check_intersection(
        &self,
        request: Request<grpc_server::CheckIntersectionRequest>,
    ) -> Result<Response<grpc_server::CheckIntersectionResponse>, Status> {
        grpc_debug!("entry.");
        let request = request.into_inner();

        let time_start: DateTime<Utc> = request
            .time_start
            .ok_or_else(|| {
                Status::invalid_argument("time_start is required for check_intersection")
            })?
            .into();

        let time_end: DateTime<Utc> = request
            .time_end
            .ok_or_else(|| Status::invalid_argument("time_end is required for check_intersection"))?
            .into();

        let pool = DEADPOOL_POSTGIS.get().ok_or_else(|| {
            grpc_error!("could not get psql pool.");
            Status::internal("could not get psql pool")
        })?;

        let client = pool.get().await.map_err(|e| {
            grpc_error!("could not get client from psql connection pool: {}", e);
            Status::internal(e.to_string())
        })?;

        let points: Vec<PointZ> = request
            .path
            .into_iter()
            .map(|p| {
                PointZ::new(
                    p.latitude,
                    p.longitude,
                    p.altitude_meters as f64,
                    Some(DEFAULT_SRID),
                )
            })
            .collect();

        let distance = points
            .windows(2)
            .fold(0.0, |acc, pair| acc + distance_meters(&pair[0], &pair[1]));

        let intersects = match best_path::intersection_checks(
            &client,
            points,
            distance,
            time_start,
            time_end,
            &request.origin_identifier,
            &request.target_identifier,
        )
        .await
        {
            Ok(()) => false,
            Err(PostgisError::BestPath(PathError::ZoneIntersection)) => true,
            Err(PostgisError::BestPath(PathError::FlightPlanIntersection)) => true,
            Err(_) => {
                grpc_error!("error checking intersection.");
                return Err(Status::internal("error checking intersection"));
            }
        };

        Ok(Response::new(grpc_server::CheckIntersectionResponse {
            intersects,
        }))
    }

    async fn get_flights(
        &self,
        request: Request<grpc_server::GetFlightsRequest>,
    ) -> Result<Response<grpc_server::GetFlightsResponse>, Status> {
        grpc_debug!("entry.");
        let request = request.into_inner();

        let flights = flight::get_flights(request).await.map_err(|e| {
            grpc_error!("error getting flights.");
            Status::internal(e.to_string())
        })?;

        let response = grpc_server::GetFlightsResponse { flights };
        Ok(Response::new(response))
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
///     let config = Config::try_from_env().unwrap();
///     tokio::spawn(grpc_server(config, None)).await
/// }
/// ```
pub async fn grpc_server(
    config: crate::config::Config,
    shutdown_rx: Option<tokio::sync::oneshot::Receiver<()>>,
) {
    grpc_debug!("entry.");

    // Grpc Server
    let grpc_port = config.docker_port_grpc;
    let full_grpc_addr: SocketAddr = match format!("[::]:{}", grpc_port).parse() {
        Ok(addr) => addr,
        Err(e) => {
            grpc_error!("Failed to parse gRPC address: {}", e);
            return;
        }
    };

    let imp = ServerImpl {};
    let (mut health_reporter, health_service) = tonic_health::server::health_reporter();
    health_reporter
        .set_serving::<RpcServiceServer<ServerImpl>>()
        .await;

    //start server
    grpc_info!("Starting gRPC services on: {}.", full_grpc_addr);
    match Server::builder()
        .add_service(health_service)
        .add_service(RpcServiceServer::new(imp))
        .serve_with_shutdown(full_grpc_addr, shutdown_signal("grpc", shutdown_rx))
        .await
    {
        Ok(_) => grpc_info!("gRPC server running at: {}.", full_grpc_addr),
        Err(e) => {
            grpc_error!("Could not start gRPC server: {}", e);
        }
    };
}

#[cfg(feature = "stub_server")]
#[tonic::async_trait]
impl RpcService for ServerImpl {
    async fn is_ready(
        &self,
        _request: Request<ReadyRequest>,
    ) -> Result<Response<ReadyResponse>, Status> {
        grpc_warn!("(MOCK) entry.");
        let response = ReadyResponse { ready: true };
        Ok(Response::new(response))
    }

    async fn update_vertiports(
        &self,
        _request: Request<grpc_server::UpdateVertiportsRequest>,
    ) -> Result<Response<grpc_server::UpdateResponse>, Status> {
        grpc_warn!("(MOCK) entry.");

        Ok(Response::new(grpc_server::UpdateResponse { updated: true }))
    }

    async fn update_waypoints(
        &self,
        _request: Request<grpc_server::UpdateWaypointsRequest>,
    ) -> Result<Response<grpc_server::UpdateResponse>, Status> {
        grpc_warn!("(MOCK) entry.");

        Ok(Response::new(grpc_server::UpdateResponse { updated: true }))
    }

    async fn update_zones(
        &self,
        _request: Request<grpc_server::UpdateZonesRequest>,
    ) -> Result<Response<grpc_server::UpdateResponse>, Status> {
        grpc_warn!("(MOCK) entry.");

        Ok(Response::new(grpc_server::UpdateResponse { updated: true }))
    }

    async fn update_flight_path(
        &self,
        _request: Request<grpc_server::UpdateFlightPathRequest>,
    ) -> Result<Response<grpc_server::UpdateResponse>, Status> {
        grpc_debug!("(MOCK) entry.");

        Ok(Response::new(grpc_server::UpdateResponse { updated: true }))
    }

    async fn best_path(
        &self,
        request: Request<grpc_server::BestPathRequest>,
    ) -> Result<Response<grpc_server::BestPathResponse>, Status> {
        grpc_warn!("(MOCK) entry.");
        let request = request.into_inner();
        let paths = best_path::best_path(request).await.map_err(|e| {
            grpc_error!("(MOCK) error getting best path.");
            Status::internal(e.to_string())
        })?;

        Ok(Response::new(grpc_server::BestPathResponse { paths }))
    }

    async fn check_intersection(
        &self,
        request: Request<grpc_server::CheckIntersectionRequest>,
    ) -> Result<Response<grpc_server::CheckIntersectionResponse>, Status> {
        grpc_warn!("(MOCK) entry.");
        let request = request.into_inner();

        let time_start: DateTime<Utc> = request
            .time_start
            .ok_or_else(|| {
                Status::invalid_argument("time_start is required for check_intersection")
            })?
            .into();

        let time_end: DateTime<Utc> = request
            .time_end
            .ok_or_else(|| Status::invalid_argument("time_end is required for check_intersection"))?
            .into();

        let pool = DEADPOOL_POSTGIS.get().ok_or_else(|| {
            grpc_error!("(MOCK) could not get psql pool.");
            Status::internal("could not get psql pool")
        })?;

        let client = pool.get().await.map_err(|e| {
            grpc_error!(
                "(MOCK) could not get client from psql connection pool: {}",
                e
            );
            Status::internal(e.to_string())
        })?;

        let points: Vec<PointZ> = request
            .path
            .into_iter()
            .map(|p| {
                PointZ::new(
                    p.latitude,
                    p.longitude,
                    p.altitude_meters as f64,
                    Some(DEFAULT_SRID),
                )
            })
            .collect();

        let distance = points
            .windows(2)
            .fold(0.0, |acc, pair| acc + distance_meters(&pair[0], &pair[1]));

        let intersects = match best_path::intersection_checks(
            &client,
            points,
            distance,
            time_start,
            time_end,
            &request.origin_identifier,
            &request.target_identifier,
        )
        .await
        {
            Ok(()) => false,
            Err(PostgisError::BestPath(PathError::ZoneIntersection)) => true,
            Err(PostgisError::BestPath(PathError::FlightPlanIntersection)) => true,
            Err(_) => {
                grpc_error!("(MOCK) error checking intersection.");
                return Err(Status::internal("error checking intersection"));
            }
        };

        Ok(Response::new(grpc_server::CheckIntersectionResponse {
            intersects,
        }))
    }

    async fn get_flights(
        &self,
        request: Request<grpc_server::GetFlightsRequest>,
    ) -> Result<Response<grpc_server::GetFlightsResponse>, Status> {
        grpc_warn!("(MOCK) entry.");
        let request = request.into_inner();

        let flights = flight::get_flights(request).await.map_err(|e| {
            grpc_error!("(MOCK) error getting flights.");
            Status::internal(e.to_string())
        })?;

        let response = grpc_server::GetFlightsResponse { flights };
        Ok(Response::new(response))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_grpc_server_is_ready() {
        let imp = ServerImpl {};
        let result = imp.is_ready(Request::new(ReadyRequest {})).await;
        assert!(result.is_ok());
        let result: ReadyResponse = result.unwrap().into_inner();
        assert_eq!(result.ready, true);
    }

    #[tokio::test]
    async fn test_grpc_server_start_and_shutdown() {
        use tokio::time::{sleep, Duration};
        lib_common::logger::get_log_handle().await;
        ut_info!("start");

        let config = crate::config::Config::default();

        let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel::<()>();

        // Start the grpc server
        tokio::spawn(grpc_server(config, Some(shutdown_rx)));

        // Give the server time to get through the startup sequence (and thus code)
        sleep(Duration::from_secs(1)).await;

        // Shut down server
        assert!(shutdown_tx.send(()).is_ok());

        ut_info!("success");
    }
}
