/// Ready Request object
///
/// No arguments
#[derive(Eq, Copy)]
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ReadyRequest {}
/// Ready Response object
#[derive(Eq, Copy)]
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ReadyResponse {
    /// True if ready
    #[prost(bool, tag = "1")]
    pub ready: bool,
}
/// Geospatial Coordinates
#[derive(Copy)]
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Coordinates {
    /// Latitude Coordinate
    #[prost(float, tag = "1")]
    pub latitude: f32,
    /// Longitude Coordinate
    #[prost(float, tag = "2")]
    pub longitude: f32,
}
/// Points in space used for routing (waypoints, vertiports, etc.)
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Node {
    /// Unique Arrow ID
    #[prost(string, tag = "1")]
    pub uuid: ::prost::alloc::string::String,
    /// Latitude Coordinate
    #[prost(message, optional, tag = "2")]
    pub location: ::core::option::Option<Coordinates>,
    /// Node Type
    #[prost(enumeration = "NodeType", tag = "3")]
    pub node_type: i32,
}
/// Update Nodes Request object
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct UpdateNodesRequest {
    /// Nodes to update
    #[prost(message, repeated, tag = "1")]
    pub nodes: ::prost::alloc::vec::Vec<Node>,
}
/// Update Nodes Response object
#[derive(Eq, Copy)]
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct UpdateNodesResponse {
    /// True if updated
    #[prost(bool, tag = "1")]
    pub updated: bool,
}
/// Points in space used for routing (waypoints, vertiports, etc.)
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct NoFlyZone {
    /// Unique label (NOTAM id, etc.)
    #[prost(string, tag = "1")]
    pub label: ::prost::alloc::string::String,
    /// Vertices bounding the No-Fly Zone
    /// The first vertex should match the end vertex (closed shape)
    #[prost(message, repeated, tag = "2")]
    pub vertices: ::prost::alloc::vec::Vec<Coordinates>,
    /// Start datetime for this zone
    #[prost(message, optional, tag = "3")]
    pub time_start: ::core::option::Option<::prost_types::Timestamp>,
    /// End datetime for this zone
    #[prost(message, optional, tag = "4")]
    pub time_end: ::core::option::Option<::prost_types::Timestamp>,
    /// If this is a no-fly centered around a vertiport, provide UUID
    #[prost(string, optional, tag = "5")]
    pub vertiport_id: ::core::option::Option<::prost::alloc::string::String>,
}
/// Update No Fly Zones Request object
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct UpdateNoFlyZonesRequest {
    /// Nodes to update
    #[prost(message, repeated, tag = "1")]
    pub zones: ::prost::alloc::vec::Vec<NoFlyZone>,
}
/// Update No Fly Zones Response object
#[derive(Copy)]
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct UpdateNoFlyZonesResponse {
    /// True if updated
    #[prost(bool, tag = "1")]
    pub updated: bool,
}
/// A path between nodes has >= 1 straight segments
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct PathSegment {
    /// Segment Index
    #[prost(int32, tag = "1")]
    pub index: i32,
    /// Start Node UUID
    #[prost(string, tag = "2")]
    pub node_uuid_start: ::prost::alloc::string::String,
    /// End Node UUID
    #[prost(string, tag = "3")]
    pub node_uuid_end: ::prost::alloc::string::String,
    /// Distance
    #[prost(double, tag = "4")]
    pub distance_meters: f64,
    /// Altitude
    #[prost(double, tag = "5")]
    pub altitude_meters: f64,
}
/// Best Path Request object
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct BestPathRequest {
    /// Start Node
    #[prost(string, tag = "1")]
    pub node_uuid_start: ::prost::alloc::string::String,
    /// End Node
    #[prost(string, tag = "2")]
    pub node_uuid_end: ::prost::alloc::string::String,
    /// Time of departure
    #[prost(message, optional, tag = "3")]
    pub time_start: ::core::option::Option<::prost_types::Timestamp>,
    /// Time of arrival
    #[prost(message, optional, tag = "4")]
    pub time_end: ::core::option::Option<::prost_types::Timestamp>,
}
/// Best Path Response object
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct BestPathResponse {
    /// Nodes in the best path
    #[prost(message, repeated, tag = "1")]
    pub segments: ::prost::alloc::vec::Vec<PathSegment>,
}
/// Node Type
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, ::prost::Enumeration)]
#[repr(i32)]
pub enum NodeType {
    /// Waypoint
    Waypoint = 0,
    /// Vertiport
    Vertiport = 1,
}
impl NodeType {
    /// String value of the enum field names used in the ProtoBuf definition.
    ///
    /// The values are not transformed in any way and thus are considered stable
    /// (if the ProtoBuf definition does not change) and safe for programmatic use.
    pub fn as_str_name(&self) -> &'static str {
        match self {
            NodeType::Waypoint => "WAYPOINT",
            NodeType::Vertiport => "VERTIPORT",
        }
    }
    /// Creates an enum from field names used in the ProtoBuf definition.
    pub fn from_str_name(value: &str) -> ::core::option::Option<Self> {
        match value {
            "WAYPOINT" => Some(Self::Waypoint),
            "VERTIPORT" => Some(Self::Vertiport),
            _ => None,
        }
    }
}
/// Generated client implementations.
pub mod rpc_service_client {
    #![allow(unused_variables, dead_code, missing_docs, clippy::let_unit_value)]
    use tonic::codegen::*;
    use tonic::codegen::http::Uri;
    /// Heartbeat
    #[derive(Debug, Clone)]
    pub struct RpcServiceClient<T> {
        inner: tonic::client::Grpc<T>,
    }
    impl RpcServiceClient<tonic::transport::Channel> {
        /// Attempt to create a new client by connecting to a given endpoint.
        pub async fn connect<D>(dst: D) -> Result<Self, tonic::transport::Error>
        where
            D: std::convert::TryInto<tonic::transport::Endpoint>,
            D::Error: Into<StdError>,
        {
            let conn = tonic::transport::Endpoint::new(dst)?.connect().await?;
            Ok(Self::new(conn))
        }
    }
    impl<T> RpcServiceClient<T>
    where
        T: tonic::client::GrpcService<tonic::body::BoxBody>,
        T::Error: Into<StdError>,
        T::ResponseBody: Body<Data = Bytes> + Send + 'static,
        <T::ResponseBody as Body>::Error: Into<StdError> + Send,
    {
        pub fn new(inner: T) -> Self {
            let inner = tonic::client::Grpc::new(inner);
            Self { inner }
        }
        pub fn with_origin(inner: T, origin: Uri) -> Self {
            let inner = tonic::client::Grpc::with_origin(inner, origin);
            Self { inner }
        }
        pub fn with_interceptor<F>(
            inner: T,
            interceptor: F,
        ) -> RpcServiceClient<InterceptedService<T, F>>
        where
            F: tonic::service::Interceptor,
            T::ResponseBody: Default,
            T: tonic::codegen::Service<
                http::Request<tonic::body::BoxBody>,
                Response = http::Response<
                    <T as tonic::client::GrpcService<tonic::body::BoxBody>>::ResponseBody,
                >,
            >,
            <T as tonic::codegen::Service<
                http::Request<tonic::body::BoxBody>,
            >>::Error: Into<StdError> + Send + Sync,
        {
            RpcServiceClient::new(InterceptedService::new(inner, interceptor))
        }
        /// Compress requests with the given encoding.
        ///
        /// This requires the server to support it otherwise it might respond with an
        /// error.
        #[must_use]
        pub fn send_compressed(mut self, encoding: CompressionEncoding) -> Self {
            self.inner = self.inner.send_compressed(encoding);
            self
        }
        /// Enable decompressing responses.
        #[must_use]
        pub fn accept_compressed(mut self, encoding: CompressionEncoding) -> Self {
            self.inner = self.inner.accept_compressed(encoding);
            self
        }
        /// Common Interfaces
        pub async fn is_ready(
            &mut self,
            request: impl tonic::IntoRequest<super::ReadyRequest>,
        ) -> Result<tonic::Response<super::ReadyResponse>, tonic::Status> {
            self.inner
                .ready()
                .await
                .map_err(|e| {
                    tonic::Status::new(
                        tonic::Code::Unknown,
                        format!("Service was not ready: {}", e.into()),
                    )
                })?;
            let codec = tonic::codec::ProstCodec::default();
            let path = http::uri::PathAndQuery::from_static("/grpc.RpcService/isReady");
            self.inner.unary(request.into_request(), path, codec).await
        }
        pub async fn update_nodes(
            &mut self,
            request: impl tonic::IntoRequest<super::UpdateNodesRequest>,
        ) -> Result<tonic::Response<super::UpdateNodesResponse>, tonic::Status> {
            self.inner
                .ready()
                .await
                .map_err(|e| {
                    tonic::Status::new(
                        tonic::Code::Unknown,
                        format!("Service was not ready: {}", e.into()),
                    )
                })?;
            let codec = tonic::codec::ProstCodec::default();
            let path = http::uri::PathAndQuery::from_static(
                "/grpc.RpcService/updateNodes",
            );
            self.inner.unary(request.into_request(), path, codec).await
        }
        pub async fn update_no_fly_zones(
            &mut self,
            request: impl tonic::IntoRequest<super::UpdateNoFlyZonesRequest>,
        ) -> Result<tonic::Response<super::UpdateNoFlyZonesResponse>, tonic::Status> {
            self.inner
                .ready()
                .await
                .map_err(|e| {
                    tonic::Status::new(
                        tonic::Code::Unknown,
                        format!("Service was not ready: {}", e.into()),
                    )
                })?;
            let codec = tonic::codec::ProstCodec::default();
            let path = http::uri::PathAndQuery::from_static(
                "/grpc.RpcService/updateNoFlyZones",
            );
            self.inner.unary(request.into_request(), path, codec).await
        }
        pub async fn best_path(
            &mut self,
            request: impl tonic::IntoRequest<super::BestPathRequest>,
        ) -> Result<tonic::Response<super::BestPathResponse>, tonic::Status> {
            self.inner
                .ready()
                .await
                .map_err(|e| {
                    tonic::Status::new(
                        tonic::Code::Unknown,
                        format!("Service was not ready: {}", e.into()),
                    )
                })?;
            let codec = tonic::codec::ProstCodec::default();
            let path = http::uri::PathAndQuery::from_static("/grpc.RpcService/bestPath");
            self.inner.unary(request.into_request(), path, codec).await
        }
    }
}
