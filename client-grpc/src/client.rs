//! Client Library: Client Functions, Structs, Traits
#![allow(unused_qualifications)]
include!("grpc.rs");

use super::*;

#[cfg(any(not(feature = "stub_client"), feature = "stub_backends"))]
use lib_common::grpc::ClientConnect;
use lib_common::grpc::{Client, GrpcClient};
use rpc_service_client::RpcServiceClient;
/// GrpcClient implementation of the RpcServiceClient
pub type GisClient = GrpcClient<RpcServiceClient<Channel>>;

cfg_if::cfg_if! {
    if #[cfg(feature = "stub_backends")] {
        use svc_gis::grpc::server::{RpcServiceServer, ServerImpl};
        use deadpool_postgres::{Config, ManagerConfig, RecyclingMethod, Runtime};
        use tokio_postgres::NoTls;

        #[tonic::async_trait]
        impl lib_common::grpc::ClientConnect<RpcServiceClient<Channel>> for GisClient {
            /// Get a connected client object
            async fn connect(
                &self,
            ) -> Result<RpcServiceClient<Channel>, tonic::transport::Error> {
                let (client, server) = tokio::io::duplex(1024);
                let mut cfg = Config::new();
                cfg.dbname = Some("deadpool".to_string());
                cfg.manager = Some(ManagerConfig { recycling_method: RecyclingMethod::Fast });

                let _pool = cfg.create_pool(Some(Runtime::Tokio1), NoTls).unwrap();
                let grpc_service = ServerImpl { };
                lib_common::grpc::mock::start_mock_server(
                    server,
                    RpcServiceServer::new(grpc_service),
                )
                .await?;

                // Move client to an option so we can _move_ the inner value
                // on the first attempt to connect. All other attempts will fail.
                let mut client = Some(client);
                let channel = tonic::transport::Endpoint::try_from("http://[::]:50051")?
                    .connect_with_connector(tower::service_fn(move |_: tonic::transport::Uri| {
                        let client = client.take();

                        async move {
                            if let Some(client) = client {
                                Ok(client)
                            } else {
                                Err(std::io::Error::new(
                                    std::io::ErrorKind::Other,
                                    "Client already taken",
                                ))
                            }
                        }
                    }))
                    .await?;

                Ok(RpcServiceClient::new(channel))
            }
        }

        super::log_macros!("grpc", "app::client::mock::gis");
    } else {
        lib_common::grpc_client!(RpcServiceClient);
        super::log_macros!("grpc", "app::client::gis");
    }
}

#[cfg(not(feature = "stub_client"))]
#[async_trait]
impl crate::service::Client<RpcServiceClient<Channel>> for GisClient {
    type ReadyRequest = ReadyRequest;
    type ReadyResponse = ReadyResponse;

    async fn is_ready(
        &self,
        request: Self::ReadyRequest,
    ) -> Result<tonic::Response<Self::ReadyResponse>, tonic::Status> {
        grpc_info!("(is_ready) {} client.", self.get_name());
        grpc_debug!("(is_ready) request: {:?}", request);
        self.get_client().await?.is_ready(request).await
    }

    async fn update_waypoints(
        &self,
        request: UpdateWaypointsRequest,
    ) -> Result<tonic::Response<UpdateResponse>, tonic::Status> {
        grpc_info!("(update_waypoints) {} client.", self.get_name());
        grpc_debug!("(update_waypoints) request: {:?}", request);
        self.get_client().await?.update_waypoints(request).await
    }

    async fn update_vertiports(
        &self,
        request: UpdateVertiportsRequest,
    ) -> Result<tonic::Response<UpdateResponse>, tonic::Status> {
        grpc_info!("(update_vertiports) {} client.", self.get_name());
        grpc_debug!("(update_vertiports) request: {:?}", request);
        self.get_client().await?.update_vertiports(request).await
    }

    async fn update_zones(
        &self,
        request: UpdateZonesRequest,
    ) -> Result<tonic::Response<UpdateResponse>, tonic::Status> {
        grpc_info!("(update_zones) {} client.", self.get_name());
        grpc_debug!("(update_zones) request: {:?}", request);
        self.get_client().await?.update_zones(request).await
    }

    async fn update_flight_path(
        &self,
        request: UpdateFlightPathRequest,
    ) -> Result<tonic::Response<UpdateResponse>, tonic::Status> {
        grpc_info!("(update_flight_path) {} client.", self.get_name());
        grpc_debug!("(update_flight_path) request: {:?}", request);
        self.get_client().await?.update_flight_path(request).await
    }

    async fn best_path(
        &self,
        request: BestPathRequest,
    ) -> Result<tonic::Response<BestPathResponse>, tonic::Status> {
        grpc_info!("(best_path) {} client.", self.get_name());
        grpc_debug!("(best_path) request: {:?}", request);
        self.get_client().await?.best_path(request).await
    }

    async fn check_intersection(
        &self,
        request: CheckIntersectionRequest,
    ) -> Result<tonic::Response<CheckIntersectionResponse>, tonic::Status> {
        grpc_info!("(check_intersection) {} client.", self.get_name());
        grpc_debug!("(check_intersection) request: {:?}", request);
        self.get_client().await?.check_intersection(request).await
    }

    async fn get_flights(
        &self,
        request: GetFlightsRequest,
    ) -> Result<tonic::Response<GetFlightsResponse>, tonic::Status> {
        grpc_info!("(get_flights) {} client.", self.get_name());
        grpc_debug!("(get_flights) request: {:?}", request);
        self.get_client().await?.get_flights(request).await
    }

    // async fn nearest_neighbors(
    //     &self,
    //     request: NearestNeighborRequest,
    // ) -> Result<tonic::Response<NearestNeighborResponse>, tonic::Status> {
    //     grpc_info!("(nearest_neighbors) {} client.", self.get_name());
    //     grpc_debug!("(nearest_neighbors) request: {:?}", request);
    //     self.get_client().await?.nearest_neighbors(request).await
    // }
}

#[cfg(feature = "stub_client")]
#[async_trait]
impl crate::service::Client<RpcServiceClient<Channel>> for GisClient {
    type ReadyRequest = ReadyRequest;
    type ReadyResponse = ReadyResponse;

    async fn is_ready(
        &self,
        request: Self::ReadyRequest,
    ) -> Result<tonic::Response<Self::ReadyResponse>, tonic::Status> {
        grpc_warn!("(is_ready MOCK) {} client.", self.get_name());
        grpc_debug!("(is_ready MOCK) request: {:?}", request);
        Ok(tonic::Response::new(ReadyResponse { ready: true }))
    }

    async fn update_waypoints(
        &self,
        request: UpdateWaypointsRequest,
    ) -> Result<tonic::Response<UpdateResponse>, tonic::Status> {
        grpc_warn!("(update_waypoints MOCK) {} client.", self.get_name());
        grpc_debug!("(update_waypoints MOCK) request: {:?}", request);
        Ok(tonic::Response::new(UpdateResponse { updated: true }))
    }

    async fn update_vertiports(
        &self,
        request: UpdateVertiportsRequest,
    ) -> Result<tonic::Response<UpdateResponse>, tonic::Status> {
        grpc_warn!("(update_vertiports MOCK) {} client.", self.get_name());
        grpc_debug!("(update_vertiports MOCK) request: {:?}", request);
        Ok(tonic::Response::new(UpdateResponse { updated: true }))
    }

    async fn update_zones(
        &self,
        request: UpdateZonesRequest,
    ) -> Result<tonic::Response<UpdateResponse>, tonic::Status> {
        grpc_warn!("(update_zones MOCK) {} client.", self.get_name());
        grpc_debug!("(update_zones MOCK) request: {:?}", request);
        Ok(tonic::Response::new(UpdateResponse { updated: true }))
    }

    async fn update_flight_path(
        &self,
        request: UpdateFlightPathRequest,
    ) -> Result<tonic::Response<UpdateResponse>, tonic::Status> {
        grpc_warn!("(update_flight_path MOCK) {} client.", self.get_name());
        grpc_debug!("(update_flight_path MOCK) request: {:?}", request);
        Ok(tonic::Response::new(UpdateResponse { updated: true }))
    }

    async fn best_path(
        &self,
        request: BestPathRequest,
    ) -> Result<tonic::Response<BestPathResponse>, tonic::Status> {
        grpc_warn!("(best_path MOCK) {} client.", self.get_name());
        grpc_debug!("(best_path MOCK) request: {:?}", request);
        Ok(tonic::Response::new(BestPathResponse {
            paths: vec![Path {
                path: vec![PathNode {
                    index: 0,
                    node_type: NodeType::Waypoint.into(),
                    identifier: "mock waypoint".to_string(),
                    geom: Some(PointZ {
                        latitude: 0.0,
                        longitude: 0.0,
                        altitude_meters: 0.0,
                    }),
                }],
                distance_meters: 0.0,
            }],
        }))
    }

    async fn check_intersection(
        &self,
        request: CheckIntersectionRequest,
    ) -> Result<tonic::Response<CheckIntersectionResponse>, tonic::Status> {
        grpc_warn!("(check_intersection MOCK) {} client.", self.get_name());
        grpc_debug!("(check_intersection MOCK) request: {:?}", request);
        Ok(tonic::Response::new(CheckIntersectionResponse {
            intersects: false,
        }))
    }

    async fn get_flights(
        &self,
        request: GetFlightsRequest,
    ) -> Result<tonic::Response<GetFlightsResponse>, tonic::Status> {
        grpc_info!("(get_flights) {} client.", self.get_name());
        grpc_debug!("(get_flights) request: {:?}", request);
        Ok(tonic::Response::new(GetFlightsResponse {
            flights: vec![Flight {
                session_id: Some("mock flight".to_string()),
                aircraft_id: Some("mock aircraft".to_string()),
                positions: vec![TimePosition {
                    position: Some(PointZ {
                        latitude: 52.64248776887166,
                        longitude: 5.11111373021763,
                        altitude_meters: 50.0,
                    }),
                    timestamp: Some(chrono::Utc::now().into()),
                }],
                simulated: true,
                aircraft_type: crate::prelude::AircraftType::Undeclared.into(),
                state: Some(crate::AircraftState {
                    timestamp: Some(chrono::Utc::now().into()),
                    status: crate::prelude::OperationalStatus::Undeclared.into(),
                    position: Some(PointZ {
                        latitude: 52.64248776887166,
                        longitude: 5.11111373021763,
                        altitude_meters: 50.0,
                    }),
                    track_angle_degrees: 12.0,
                    ground_speed_mps: 5.0,
                    vertical_speed_mps: 1.0,
                }),
            }],
            // isas: vec![],
        }))
    }

    // async fn nearest_neighbors(
    //     &self,
    //     request: NearestNeighborRequest,
    // ) -> Result<tonic::Response<NearestNeighborResponse>, tonic::Status> {
    //     grpc_info!("(nearest_neighbors MOCK) {} client.", self.get_name());
    //     grpc_debug!("(nearest_neighbors MOCK) request: {:?}", request);
    //     Ok(tonic::Response::new(NearestNeighborResponse {
    //         distances: vec![DistanceTo {
    //             label: "mock vertiport".to_string(),
    //             target_type: request.origin_type,
    //             distance_meters: 500.0,
    //         }],
    //     }))
    // }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::service::Client as ServiceClient;
    use tonic::transport::Channel;

    fn get_client() -> GrpcClient<RpcServiceClient<Channel>> {
        let name = "gis";
        let (server_host, server_port) =
            lib_common::grpc::get_endpoint_from_env("GRPC_HOST", "GRPC_PORT");

        GrpcClient::new_client(&server_host, server_port, name)
    }

    #[tokio::test]
    #[cfg(not(feature = "stub_client"))]
    async fn test_client_connect() {
        let client = get_client();
        let connection = client.get_client().await;
        println!("{:?}", connection);
        assert!(connection.is_ok());
    }

    #[tokio::test]
    async fn test_client_is_ready_request() {
        let client = get_client();
        let result = client.is_ready(ReadyRequest {}).await;
        println!("{:?}", result);
        assert!(result.is_ok());
        assert_eq!(result.unwrap().into_inner().ready, true);
    }
}
