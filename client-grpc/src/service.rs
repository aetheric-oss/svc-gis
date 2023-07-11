//! Client Library: Client Functions, Structs, Traits

use super::client::{
    BestPathRequest, BestPathResponse, UpdateAircraftPositionRequest, UpdateNoFlyZonesRequest,
    UpdateResponse, UpdateVertiportsRequest, UpdateWaypointsRequest,
};
use tonic::{Request, Response, Status};

/// gRPC object traits to provide wrappers for grpc functions
#[tonic::async_trait]
pub trait Client<T>
where
    Self: Sized + lib_common::grpc::Client<T> + lib_common::grpc::ClientConnect<T>,
    T: Send + Clone,
{
    /// The type expected for ReadyRequest structs.
    type ReadyRequest;
    /// The type expected for ReadyResponse structs.
    type ReadyResponse;

    /// Returns a [`tonic::Response`] containing a [`ReadyResponse`]
    /// Takes an [`ReadyRequest`](Self::ReadyRequest).
    ///
    /// # Errors
    ///
    /// Returns [`tonic::Status`] with [`Code::Unknown`](tonic::Code::Unknown) if
    /// the server is not ready.
    ///
    /// # Examples
    /// ```
    /// use lib_common::grpc::get_endpoint_from_env;
    /// use svc_gis_client_grpc::client::{ReadyRequest, RpcServiceClient};
    /// use svc_gis_client_grpc::{Client, GrpcClient};
    /// use svc_gis_client_grpc::service::Client as ServiceClient;
    /// use tonic::transport::Channel;
    ///
    /// async fn example () -> Result<(), Box<dyn std::error::Error>> {
    ///     let (host, port) = get_endpoint_from_env("SERVER_HOSTNAME", "SERVER_PORT_GRPC");
    ///     let connection = GrpcClient::<RpcServiceClient<Channel>>::new_client(&host, port, "gis");
    ///     let response = connection
    ///         .is_ready(ReadyRequest {})
    ///         .await?;
    ///     println!("RESPONSE={:?}", response.into_inner());
    ///     Ok(())
    /// }
    /// ```
    async fn is_ready(
        &self,
        request: Self::ReadyRequest,
    ) -> Result<Response<Self::ReadyResponse>, Status>;

    /// Returns a [`tonic::Response`] containing a [`UpdateResponse`]
    /// Takes an [`UpdateWaypointsRequest`](UpdateWaypointsRequest).
    ///
    /// # Errors
    ///
    /// Returns [`tonic::Status`] with [`Code::Unknown`](tonic::Code::Unknown) if
    /// the server is not ready.
    ///
    /// # Examples
    /// ```
    /// use lib_common::grpc::get_endpoint_from_env;
    /// use svc_gis_client_grpc::client::{ReadyRequest, RpcServiceClient};
    /// use svc_gis_client_grpc::client::{UpdateWaypointsRequest, UpdateResponse};
    /// use svc_gis_client_grpc::{Client, GrpcClient};
    /// use svc_gis_client_grpc::service::Client as ServiceClient;
    /// use tonic::transport::Channel;
    ///
    /// async fn example () -> Result<(), Box<dyn std::error::Error>> {
    ///     let (host, port) = get_endpoint_from_env("SERVER_HOSTNAME", "SERVER_PORT_GRPC");
    ///     let connection = GrpcClient::<RpcServiceClient<Channel>>::new_client(&host, port, "gis");
    ///     let request = tonic::Request::new(UpdateWaypointsRequest { waypoints: vec![] });
    ///     let response = connection.update_waypoints(request).await?;
    ///     println!("RESPONSE={:?}", response.into_inner());
    ///     Ok(())
    /// }
    /// ```
    async fn update_waypoints(
        &self,
        request: Request<UpdateWaypointsRequest>,
    ) -> Result<Response<UpdateResponse>, Status>;

    /// Returns a [`tonic::Response`] containing a [`UpdateResponse`]
    /// Takes an [`UpdateVertiportsRequest`](UpdateVertiportsRequest).
    ///
    /// # Errors
    ///
    /// Returns [`tonic::Status`] with [`Code::Unknown`](tonic::Code::Unknown) if
    /// the server is not ready.
    ///
    /// # Examples
    /// ```
    /// use lib_common::grpc::get_endpoint_from_env;
    /// use svc_gis_client_grpc::client::{ReadyRequest, RpcServiceClient};
    /// use svc_gis_client_grpc::client::{UpdateVertiportsRequest, UpdateResponse};
    /// use svc_gis_client_grpc::{Client, GrpcClient};
    /// use svc_gis_client_grpc::service::Client as ServiceClient;
    /// use tonic::transport::Channel;
    ///
    /// async fn example () -> Result<(), Box<dyn std::error::Error>> {
    ///     let (host, port) = get_endpoint_from_env("SERVER_HOSTNAME", "SERVER_PORT_GRPC");
    ///     let connection = GrpcClient::<RpcServiceClient<Channel>>::new_client(&host, port, "gis");
    ///     let request = tonic::Request::new(UpdateVertiportsRequest { vertiports: vec![] });
    ///     let response = connection.update_vertiports(request).await?;
    ///     println!("RESPONSE={:?}", response.into_inner());
    ///     Ok(())
    /// }
    /// ```
    async fn update_vertiports(
        &self,
        request: Request<UpdateVertiportsRequest>,
    ) -> Result<Response<UpdateResponse>, Status>;

    /// Returns a [`tonic::Response`] containing a [`UpdateResponse`]
    /// Takes an [`UpdateNoFlyZonesRequest`](UpdateNoFlyZonesRequest).
    ///
    /// # Errors
    ///
    /// Returns [`tonic::Status`] with [`Code::Unknown`](tonic::Code::Unknown) if
    /// the server is not ready.
    ///
    /// # Examples
    /// ```
    /// use lib_common::grpc::get_endpoint_from_env;
    /// use svc_gis_client_grpc::client::{ReadyRequest, RpcServiceClient};
    /// use svc_gis_client_grpc::client::{UpdateNoFlyZonesRequest, UpdateResponse};
    /// use svc_gis_client_grpc::{Client, GrpcClient};
    /// use svc_gis_client_grpc::service::Client as ServiceClient;
    /// use tonic::transport::Channel;
    ///
    /// async fn example () -> Result<(), Box<dyn std::error::Error>> {
    ///     let (host, port) = get_endpoint_from_env("SERVER_HOSTNAME", "SERVER_PORT_GRPC");
    ///     let connection = GrpcClient::<RpcServiceClient<Channel>>::new_client(&host, port, "gis");
    ///     let request = tonic::Request::new(UpdateNoFlyZonesRequest { zones: vec![] });
    ///     let response = connection.update_no_fly_zones(request).await?;
    ///     println!("RESPONSE={:?}", response.into_inner());
    ///     Ok(())
    /// }
    /// ```
    async fn update_no_fly_zones(
        &self,
        request: Request<UpdateNoFlyZonesRequest>,
    ) -> Result<Response<UpdateResponse>, Status>;

    /// Returns a [`tonic::Response`] containing a [`UpdateResponse`]
    /// Takes an [`UpdateAircraftPositionRequest`](UpdateAircraftPositionRequest).
    ///
    /// # Errors
    ///
    /// Returns [`tonic::Status`] with [`Code::Unknown`](tonic::Code::Unknown) if
    /// the server is not ready.
    ///
    /// # Examples
    /// ```
    /// use lib_common::grpc::get_endpoint_from_env;
    /// use svc_gis_client_grpc::client::RpcServiceClient;
    /// use svc_gis_client_grpc::client::{UpdateAircraftPositionRequest, UpdateResponse};
    /// use svc_gis_client_grpc::{Client, GrpcClient};
    /// use svc_gis_client_grpc::service::Client as ServiceClient;
    /// use tonic::transport::Channel;
    ///
    /// async fn example () -> Result<(), Box<dyn std::error::Error>> {
    ///     let (host, port) = get_endpoint_from_env("SERVER_HOSTNAME", "SERVER_PORT_GRPC");
    ///     let connection = GrpcClient::<RpcServiceClient<Channel>>::new_client(&host, port, "gis");
    ///     let request = tonic::Request::new(UpdateAircraftPositionRequest { aircraft: vec![] });
    ///     let response = connection.update_aircraft_position(request).await?;
    ///     println!("RESPONSE={:?}", response.into_inner());
    ///     Ok(())
    /// }
    /// ```
    async fn update_aircraft_position(
        &self,
        request: Request<UpdateAircraftPositionRequest>,
    ) -> Result<Response<UpdateResponse>, Status>;

    /// Returns a [`tonic::Response`] containing a [`UpdateResponse`]
    /// Takes an [`BestPathRequest`](BestPathRequest).
    ///
    /// # Errors
    ///
    /// Returns [`tonic::Status`] with [`Code::Unknown`](tonic::Code::Unknown) if
    /// the server is not ready.
    ///
    /// # Examples
    /// ```
    /// use lib_common::grpc::get_endpoint_from_env;
    /// use lib_common::time::datetime_to_timestamp;
    /// use svc_gis_client_grpc::client::RpcServiceClient;
    /// use svc_gis_client_grpc::client::{BestPathRequest, BestPathResponse};
    /// use svc_gis_client_grpc::{Client, GrpcClient};
    /// use svc_gis_client_grpc::service::Client as ServiceClient;
    /// use tonic::transport::Channel;
    ///
    /// async fn example () -> Result<(), Box<dyn std::error::Error>> {
    ///     let (host, port) = get_endpoint_from_env("SERVER_HOSTNAME", "SERVER_PORT_GRPC");
    ///     let connection = GrpcClient::<RpcServiceClient<Channel>>::new_client(&host, port, "gis");
    ///     let request = tonic::Request::new(BestPathRequest {
    ///         node_uuid_start: "".to_string(),
    ///         node_uuid_end: "".to_string(),
    ///         start_type: 0,
    ///         time_start: datetime_to_timestamp(&chrono::Utc::now()),
    ///         time_end: datetime_to_timestamp(&chrono::Utc::now()),
    ///     });
    ///     let response = connection.best_path(request).await?;
    ///     println!("RESPONSE={:?}", response.into_inner());
    ///     Ok(())
    /// }
    /// ```
    async fn best_path(
        &self,
        request: Request<BestPathRequest>,
    ) -> Result<Response<BestPathResponse>, Status>;
}
