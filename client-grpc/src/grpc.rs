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
/// General update response object
#[derive(Eq, Copy)]
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct UpdateResponse {
    /// True if updated
    #[prost(bool, tag = "1")]
    pub updated: bool,
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
/// Vertiport Type
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Vertiport {
    /// Unique Arrow ID
    #[prost(string, tag = "1")]
    pub uuid: ::prost::alloc::string::String,
    /// Vertiport Polygon
    #[prost(message, repeated, tag = "2")]
    pub vertices: ::prost::alloc::vec::Vec<Coordinates>,
    /// Vertiport Label
    #[prost(string, optional, tag = "3")]
    pub label: ::core::option::Option<::prost::alloc::string::String>,
}
/// Waypoint Type
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Waypoint {
    /// Unique label
    #[prost(string, tag = "1")]
    pub label: ::prost::alloc::string::String,
    /// Latitude Coordinate
    #[prost(message, optional, tag = "2")]
    pub location: ::core::option::Option<Coordinates>,
}
/// Aircraft Type
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct AircraftPosition {
    /// Aircraft Callsign
    #[prost(string, tag = "1")]
    pub callsign: ::prost::alloc::string::String,
    /// Aircraft Location
    #[prost(message, optional, tag = "2")]
    pub location: ::core::option::Option<Coordinates>,
    /// Aircraft Altitude
    #[prost(float, tag = "3")]
    pub altitude_meters: f32,
    /// Telemetry Report Time
    #[prost(message, optional, tag = "4")]
    pub time: ::core::option::Option<::prost_wkt_types::Timestamp>,
    /// Aircraft UUID, if available
    #[prost(string, optional, tag = "5")]
    pub uuid: ::core::option::Option<::prost::alloc::string::String>,
}
/// Update Vertiports Request object
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct UpdateVertiportsRequest {
    /// Nodes to update
    #[prost(message, repeated, tag = "1")]
    pub vertiports: ::prost::alloc::vec::Vec<Vertiport>,
}
/// Update Waypoints Request object
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct UpdateWaypointsRequest {
    /// Nodes to update
    #[prost(message, repeated, tag = "1")]
    pub waypoints: ::prost::alloc::vec::Vec<Waypoint>,
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
    pub time_start: ::core::option::Option<::prost_wkt_types::Timestamp>,
    /// End datetime for this zone
    #[prost(message, optional, tag = "4")]
    pub time_end: ::core::option::Option<::prost_wkt_types::Timestamp>,
}
/// Update No Fly Zones Request object
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct UpdateNoFlyZonesRequest {
    /// Nodes to update
    #[prost(message, repeated, tag = "1")]
    pub zones: ::prost::alloc::vec::Vec<NoFlyZone>,
}
/// Update Aircraft Request Object
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct UpdateAircraftPositionRequest {
    /// List of aircraft to update
    #[prost(message, repeated, tag = "1")]
    pub aircraft: ::prost::alloc::vec::Vec<AircraftPosition>,
}
/// A path between nodes has >= 1 straight segments
#[derive(Copy)]
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct PathSegment {
    /// Segment Index
    #[prost(int32, tag = "1")]
    pub index: i32,
    /// Start Node Type (Waypoint, Aircraft, or Vertiport)
    #[prost(enumeration = "NodeType", tag = "2")]
    pub start_type: i32,
    /// Latitude
    #[prost(float, tag = "3")]
    pub start_latitude: f32,
    /// Longitude
    #[prost(float, tag = "4")]
    pub start_longitude: f32,
    /// End Node Type (Vertiport or Waypoint)
    #[prost(enumeration = "NodeType", tag = "5")]
    pub end_type: i32,
    /// Latitude
    #[prost(float, tag = "6")]
    pub end_latitude: f32,
    /// Longitude
    #[prost(float, tag = "7")]
    pub end_longitude: f32,
    /// Distance
    #[prost(float, tag = "8")]
    pub distance_meters: f32,
    /// Altitude
    #[prost(float, tag = "9")]
    pub altitude_meters: f32,
}
/// Best Path Request object
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct BestPathRequest {
    /// Start Node - UUID for Vertiports, Callsigns for Aircraft
    #[prost(string, tag = "1")]
    pub node_start_id: ::prost::alloc::string::String,
    /// End Node (Vertiport UUID)
    #[prost(string, tag = "2")]
    pub node_uuid_end: ::prost::alloc::string::String,
    /// Start Node Type (Vertiport or Aircraft Allowed)
    #[prost(enumeration = "NodeType", tag = "3")]
    pub start_type: i32,
    /// Time of departure
    #[prost(message, optional, tag = "4")]
    pub time_start: ::core::option::Option<::prost_wkt_types::Timestamp>,
    /// Time of arrival
    #[prost(message, optional, tag = "5")]
    pub time_end: ::core::option::Option<::prost_wkt_types::Timestamp>,
}
/// Best Path Response object
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct BestPathResponse {
    /// Nodes in the best path
    #[prost(message, repeated, tag = "1")]
    pub segments: ::prost::alloc::vec::Vec<PathSegment>,
}
/// Nearest Neighbor Request object
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct NearestNeighborRequest {
    /// Start Node - UUID for Vertiports, Callsigns for Aircraft
    #[prost(string, tag = "1")]
    pub start_node_id: ::prost::alloc::string::String,
    /// Start Node Type (Vertiport or Aircraft Allowed)
    #[prost(enumeration = "NodeType", tag = "2")]
    pub start_type: i32,
    /// End Node Type (Vertiport or Aircraft Allowed)
    #[prost(enumeration = "NodeType", tag = "3")]
    pub end_type: i32,
    /// Limit to this many results
    #[prost(int32, tag = "4")]
    pub limit: i32,
    /// Limit to this range
    #[prost(float, tag = "5")]
    pub max_range_meters: f32,
}
/// Distance to a node
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct DistanceTo {
    /// Vertiport or Aircraft ID
    #[prost(string, tag = "1")]
    pub label: ::prost::alloc::string::String,
    /// Vertiport or Aircraft Type
    #[prost(enumeration = "NodeType", tag = "2")]
    pub target_type: i32,
    /// Distance to vertiport
    #[prost(float, tag = "3")]
    pub distance_meters: f32,
}
/// Nearest Vertiports Request object
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct NearestNeighborResponse {
    /// Distances to nearby objects
    #[prost(message, repeated, tag = "1")]
    pub distances: ::prost::alloc::vec::Vec<DistanceTo>,
}
/// Types of nodes in itinerary
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, ::prost::Enumeration)]
#[repr(i32)]
pub enum NodeType {
    /// Vertiport node
    Vertiport = 0,
    /// Waypoint node
    Waypoint = 1,
    /// Aircraft node
    Aircraft = 2,
}
impl NodeType {
    /// String value of the enum field names used in the ProtoBuf definition.
    ///
    /// The values are not transformed in any way and thus are considered stable
    /// (if the ProtoBuf definition does not change) and safe for programmatic use.
    pub fn as_str_name(&self) -> &'static str {
        match self {
            NodeType::Vertiport => "VERTIPORT",
            NodeType::Waypoint => "WAYPOINT",
            NodeType::Aircraft => "AIRCRAFT",
        }
    }
    /// Creates an enum from field names used in the ProtoBuf definition.
    pub fn from_str_name(value: &str) -> ::core::option::Option<Self> {
        match value {
            "VERTIPORT" => Some(Self::Vertiport),
            "WAYPOINT" => Some(Self::Waypoint),
            "AIRCRAFT" => Some(Self::Aircraft),
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
        pub async fn update_vertiports(
            &mut self,
            request: impl tonic::IntoRequest<super::UpdateVertiportsRequest>,
        ) -> Result<tonic::Response<super::UpdateResponse>, tonic::Status> {
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
                "/grpc.RpcService/updateVertiports",
            );
            self.inner.unary(request.into_request(), path, codec).await
        }
        pub async fn update_waypoints(
            &mut self,
            request: impl tonic::IntoRequest<super::UpdateWaypointsRequest>,
        ) -> Result<tonic::Response<super::UpdateResponse>, tonic::Status> {
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
                "/grpc.RpcService/updateWaypoints",
            );
            self.inner.unary(request.into_request(), path, codec).await
        }
        pub async fn update_no_fly_zones(
            &mut self,
            request: impl tonic::IntoRequest<super::UpdateNoFlyZonesRequest>,
        ) -> Result<tonic::Response<super::UpdateResponse>, tonic::Status> {
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
        pub async fn update_aircraft_position(
            &mut self,
            request: impl tonic::IntoRequest<super::UpdateAircraftPositionRequest>,
        ) -> Result<tonic::Response<super::UpdateResponse>, tonic::Status> {
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
                "/grpc.RpcService/updateAircraftPosition",
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
        pub async fn nearest_neighbors(
            &mut self,
            request: impl tonic::IntoRequest<super::NearestNeighborRequest>,
        ) -> Result<tonic::Response<super::NearestNeighborResponse>, tonic::Status> {
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
                "/grpc.RpcService/nearestNeighbors",
            );
            self.inner.unary(request.into_request(), path, codec).await
        }
    }
}
