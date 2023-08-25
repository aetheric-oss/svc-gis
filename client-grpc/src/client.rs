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

                let pool = cfg.create_pool(Some(Runtime::Tokio1), NoTls).unwrap();
                let grpc_service = ServerImpl { pool };
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

    async fn update_aircraft_position(
        &self,
        request: UpdateAircraftPositionRequest,
    ) -> Result<tonic::Response<UpdateResponse>, tonic::Status> {
        grpc_info!("(update_aircraft_position) {} client.", self.get_name());
        grpc_debug!("(update_aircraft_position) request: {:?}", request);
        self.get_client()
            .await?
            .update_aircraft_position(request)
            .await
    }

    async fn update_no_fly_zones(
        &self,
        request: UpdateNoFlyZonesRequest,
    ) -> Result<tonic::Response<UpdateResponse>, tonic::Status> {
        grpc_info!("(update_no_fly_zones) {} client.", self.get_name());
        grpc_debug!("(update_no_fly_zones) request: {:?}", request);
        self.get_client().await?.update_no_fly_zones(request).await
    }

    async fn best_path(
        &self,
        request: BestPathRequest,
    ) -> Result<tonic::Response<BestPathResponse>, tonic::Status> {
        grpc_info!("(best_path) {} client.", self.get_name());
        grpc_debug!("(best_path) request: {:?}", request);
        self.get_client().await?.best_path(request).await
    }

    async fn nearest_neighbors(
        &self,
        request: NearestNeighborRequest,
    ) -> Result<tonic::Response<NearestNeighborResponse>, tonic::Status> {
        grpc_info!("(nearest_neighbors) {} client.", self.get_name());
        grpc_debug!("(nearest_neighbors) request: {:?}", request);
        self.get_client().await?.nearest_neighbors(request).await
    }
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

    async fn update_no_fly_zones(
        &self,
        request: UpdateNoFlyZonesRequest,
    ) -> Result<tonic::Response<UpdateResponse>, tonic::Status> {
        grpc_warn!("(update_no_fly_zones MOCK) {} client.", self.get_name());
        grpc_debug!("(update_no_fly_zones MOCK) request: {:?}", request);
        Ok(tonic::Response::new(UpdateResponse { updated: true }))
    }

    async fn update_aircraft_position(
        &self,
        request: UpdateAircraftPositionRequest,
    ) -> Result<tonic::Response<UpdateResponse>, tonic::Status> {
        grpc_warn!(
            "(update_aircraft_position MOCK) {} client.",
            self.get_name()
        );
        grpc_debug!("(update_aircraft_position MOCK) request: {:?}", request);
        Ok(tonic::Response::new(UpdateResponse { updated: true }))
    }

    async fn best_path(
        &self,
        request: BestPathRequest,
    ) -> Result<tonic::Response<BestPathResponse>, tonic::Status> {
        grpc_warn!("(best_path MOCK) {} client.", self.get_name());
        grpc_debug!("(best_path MOCK) request: {:?}", request);
        Ok(tonic::Response::new(BestPathResponse {
            segments: vec![PathSegment {
                index: 0,
                start_type: NodeType::Vertiport as i32,
                start_latitude: 52.374746487741156,
                start_longitude: 4.916383166303402,
                end_type: NodeType::Vertiport as i32,
                end_latitude: 52.3751804160378,
                end_longitude: 4.916396577348476,
                distance_meters: 50.0,
                altitude_meters: 10.0,
            }],
        }))
    }

    async fn nearest_neighbors(
        &self,
        request: NearestNeighborRequest,
    ) -> Result<tonic::Response<NearestNeighborResponse>, tonic::Status> {
        grpc_info!("(nearest_neighbors MOCK) {} client.", self.get_name());
        grpc_debug!("(nearest_neighbors MOCK) request: {:?}", request);
        Ok(tonic::Response::new(NearestNeighborResponse {
            distances: vec![DistanceTo {
                label: "mock vertiport".to_string(),
                target_type: request.start_type,
                distance_meters: 500.0,
            }],
        }))
    }
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
