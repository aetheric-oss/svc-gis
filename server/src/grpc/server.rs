//! gRPC server implementation
/// module generated from proto/svc-template-rust-grpc.proto

pub mod grpc_server {
    #![allow(unused_qualifications, missing_docs)]
    tonic::include_proto!("grpc");
}

use crate::postgis::utils::distance_meters;
use crate::postgis::{best_path::PathError, *};
use crate::shutdown_signal;
use chrono::{DateTime, Utc};
pub use grpc_server::rpc_service_server::{RpcService, RpcServiceServer};
use grpc_server::{ReadyRequest, ReadyResponse};
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
    #[cfg(not(tarpaulin_include))]
    async fn is_ready(
        &self,
        _request: Request<ReadyRequest>,
    ) -> Result<Response<ReadyResponse>, Status> {
        grpc_debug!("(is_ready) entry.");
        let response = ReadyResponse { ready: true };
        Ok(Response::new(response))
    }

    #[cfg(not(tarpaulin_include))]
    async fn update_vertiports(
        &self,
        request: Request<grpc_server::UpdateVertiportsRequest>,
    ) -> Result<Response<grpc_server::UpdateResponse>, Status> {
        grpc_debug!("(update_vertiports) entry.");

        // Update nodes in PostGIS
        match vertiport::update_vertiports(request.into_inner().vertiports).await {
            Ok(_) => Ok(Response::new(grpc_server::UpdateResponse { updated: true })),
            Err(e) => {
                grpc_error!("(update_vertiports) error updating vertiports.");
                Err(Status::internal(e.to_string()))
            }
        }
    }

    #[cfg(not(tarpaulin_include))]
    async fn update_waypoints(
        &self,
        request: Request<grpc_server::UpdateWaypointsRequest>,
    ) -> Result<Response<grpc_server::UpdateResponse>, Status> {
        grpc_debug!("(update_waypoints) entry.");

        // Update nodes in PostGIS
        match waypoint::update_waypoints(request.into_inner().waypoints).await {
            Ok(_) => Ok(Response::new(grpc_server::UpdateResponse { updated: true })),
            Err(e) => {
                grpc_error!("(update_waypoints) error updating nodes: {}", e);
                Err(Status::internal(e.to_string()))
            }
        }
    }

    #[cfg(not(tarpaulin_include))]
    async fn update_zones(
        &self,
        request: Request<grpc_server::UpdateZonesRequest>,
    ) -> Result<Response<grpc_server::UpdateResponse>, Status> {
        grpc_debug!("(update_zones) entry.");

        // Update nodes in PostGIS
        match zone::update_zones(request.into_inner().zones).await {
            Ok(_) => Ok(Response::new(grpc_server::UpdateResponse { updated: true })),
            Err(e) => {
                grpc_error!("(update_zones) error updating zones: {}", e);
                Err(Status::internal(e.to_string()))
            }
        }
    }

    #[cfg(not(tarpaulin_include))]
    async fn update_flight_path(
        &self,
        request: Request<grpc_server::UpdateFlightPathRequest>,
    ) -> Result<Response<grpc_server::UpdateResponse>, Status> {
        grpc_debug!("(update_flight_path) entry.");

        // Update nodes in PostGIS
        match flight::update_flight_path(request.into_inner()).await {
            Ok(_) => Ok(Response::new(grpc_server::UpdateResponse { updated: true })),
            Err(e) => {
                grpc_error!("(update_flight_path) error updating flight path: {}", e);
                Err(Status::internal(e.to_string()))
            }
        }
    }

    #[cfg(not(tarpaulin_include))]
    async fn best_path(
        &self,
        request: Request<grpc_server::BestPathRequest>,
    ) -> Result<Response<grpc_server::BestPathResponse>, Status> {
        grpc_debug!("(best_path) entry.");
        let request = request.into_inner();
        match best_path::best_path(request).await {
            Ok(paths) => {
                let response = grpc_server::BestPathResponse { paths };
                Ok(Response::new(response))
            }
            Err(e) => {
                grpc_error!("(best_path) error getting best path: {}", e);
                Err(Status::internal(e.to_string()))
            }
        }
    }

    #[cfg(not(tarpaulin_include))]
    async fn check_intersection(
        &self,
        request: Request<grpc_server::CheckIntersectionRequest>,
    ) -> Result<Response<grpc_server::CheckIntersectionResponse>, Status> {
        grpc_debug!("(check_intersection) entry.");
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
            grpc_error!("(check_intersection) could not get psql pool.");
            Status::internal("could not get psql pool")
        })?;

        let client = pool.get().await.map_err(|e| {
            grpc_error!(
                "(check_intersection) could not get client from psql connection pool: {}",
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
                grpc_error!("(check_intersection) error checking intersection.");
                return Err(Status::internal("error checking intersection"));
            }
        };

        Ok(Response::new(grpc_server::CheckIntersectionResponse {
            intersects,
        }))
    }

    #[cfg(not(tarpaulin_include))]
    async fn get_flights(
        &self,
        request: Request<grpc_server::GetFlightsRequest>,
    ) -> Result<Response<grpc_server::GetFlightsResponse>, Status> {
        grpc_debug!("(get_flights) entry.");
        let request = request.into_inner();
        match flight::get_flights(request).await {
            Ok(flights) => {
                let response = grpc_server::GetFlightsResponse {
                    flights,
                    // isas: vec![],
                };
                Ok(Response::new(response))
            }
            Err(e) => {
                grpc_error!("(get_flights) error getting flights: {}", e);
                Err(Status::internal(e.to_string()))
            }
        }
    }

    // #[cfg(not(tarpaulin_include))]
    // async fn nearest_neighbors(
    //     &self,
    //     request: Request<grpc_server::NearestNeighborRequest>,
    // ) -> Result<Response<grpc_server::NearestNeighborResponse>, Status> {
    //     grpc_debug!("(nearest_neighbors) entry.");

    //     match nearest::nearest_neighbors(request.into_inner()).await {
    //         Ok(distances) => {
    //             let response = grpc_server::NearestNeighborResponse { distances };
    //             Ok(Response::new(response))
    //         }
    //         Err(e) => {
    //             grpc_error!("(nearest_neighbors) error getting nearest neighbors: {}", e);
    //             Err(Status::internal(e.to_string()))
    //         }
    //     }
    // }
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
#[cfg(not(tarpaulin_include))]
pub async fn grpc_server(
    config: crate::config::Config,
    shutdown_rx: Option<tokio::sync::oneshot::Receiver<()>>,
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

    let imp = ServerImpl {};
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
        grpc_warn!("(is_ready MOCK) entry.");
        let response = ReadyResponse { ready: true };
        Ok(Response::new(response))
    }

    #[cfg(not(tarpaulin_include))]
    async fn update_vertiports(
        &self,
        _request: Request<grpc_server::UpdateVertiportsRequest>,
    ) -> Result<Response<grpc_server::UpdateResponse>, Status> {
        grpc_warn!("(update_vertiports MOCK) entry.");

        Ok(Response::new(grpc_server::UpdateResponse { updated: true }))
    }

    #[cfg(not(tarpaulin_include))]
    async fn update_waypoints(
        &self,
        _request: Request<grpc_server::UpdateWaypointsRequest>,
    ) -> Result<Response<grpc_server::UpdateResponse>, Status> {
        grpc_warn!("(update_waypoints MOCK) entry.");

        Ok(Response::new(grpc_server::UpdateResponse { updated: true }))
    }

    #[cfg(not(tarpaulin_include))]
    async fn update_zones(
        &self,
        _request: Request<grpc_server::UpdateZonesRequest>,
    ) -> Result<Response<grpc_server::UpdateResponse>, Status> {
        grpc_warn!("(update_zones MOCK) entry.");

        Ok(Response::new(grpc_server::UpdateResponse { updated: true }))
    }

    #[cfg(not(tarpaulin_include))]
    async fn update_flight_path(
        &self,
        _request: Request<grpc_server::UpdateFlightPathRequest>,
    ) -> Result<Response<grpc_server::UpdateResponse>, Status> {
        grpc_debug!("(update_flight_path MOCK) entry.");

        Ok(Response::new(grpc_server::UpdateResponse { updated: true }))
    }

    #[cfg(not(tarpaulin_include))]
    async fn best_path(
        &self,
        request: Request<grpc_server::BestPathRequest>,
    ) -> Result<Response<grpc_server::BestPathResponse>, Status> {
        grpc_warn!("(best_path MOCK) entry.");
        let request = request.into_inner();
        match best_path::best_path(request).await {
            Ok(paths) => {
                let response = grpc_server::BestPathResponse { paths };
                Ok(Response::new(response))
            }
            Err(e) => {
                grpc_error!("(best_path MOCK) error getting best path.");
                Err(Status::internal(e.to_string()))
            }
        }
    }

    #[cfg(not(tarpaulin_include))]
    async fn check_intersection(
        &self,
        request: Request<grpc_server::CheckIntersectionRequest>,
    ) -> Result<Response<grpc_server::CheckIntersectionResponse>, Status> {
        grpc_warn!("(check_intersection MOCK) entry.");
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
            grpc_error!("(check_intersection MOCK) could not get psql pool.");
            Status::internal("could not get psql pool")
        })?;

        let client = pool.get().await.map_err(|e| {
            grpc_error!(
                "(check_intersection MOCK) could not get client from psql connection pool: {}",
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
                grpc_error!("(check_intersection MOCK) error checking intersection.");
                return Err(Status::internal("error checking intersection"));
            }
        };

        Ok(Response::new(grpc_server::CheckIntersectionResponse {
            intersects,
        }))
    }

    #[cfg(not(tarpaulin_include))]
    async fn get_flights(
        &self,
        request: Request<grpc_server::GetFlightsRequest>,
    ) -> Result<Response<grpc_server::GetFlightsResponse>, Status> {
        grpc_warn!("(get_flights MOCK) entry.");
        let request = request.into_inner();
        match flight::get_flights(request).await {
            Ok(flights) => {
                let response = grpc_server::GetFlightsResponse { flights };
                Ok(Response::new(response))
            }
            Err(e) => {
                grpc_error!("(get_flights MOCK) error getting flights.");
                Err(Status::internal(e.to_string()))
            }
        }
    }

    // #[cfg(not(tarpaulin_include))]
    // async fn nearest_neighbors(
    //     &self,
    //     request: Request<grpc_server::NearestNeighborRequest>,
    // ) -> Result<Response<grpc_server::NearestNeighborResponse>, Status> {
    //     grpc_warn!("(nearest_neighbors MOCK) entry.");
    //     match nearest::nearest_neighbors(request.into_inner()).await {
    //         Ok(distances) => {
    //             let response = grpc_server::NearestNeighborResponse { distances };
    //             Ok(Response::new(response))
    //         }
    //         Err(e) => {
    //             grpc_error!("(nearest_neighbors MOCK) error getting nearest neighbors.");
    //             Err(Status::internal(e.to_string()))
    //         }
    //     }
    // }
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
}
