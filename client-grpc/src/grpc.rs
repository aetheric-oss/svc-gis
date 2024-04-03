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
    #[prost(double, tag = "1")]
    pub latitude: f64,
    /// Longitude Coordinate
    #[prost(double, tag = "2")]
    pub longitude: f64,
}
/// Vertiport Type
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Vertiport {
    /// Unique Arrow ID
    #[prost(string, tag = "1")]
    pub identifier: ::prost::alloc::string::String,
    /// Vertiport Polygon
    #[prost(message, repeated, tag = "2")]
    pub vertices: ::prost::alloc::vec::Vec<Coordinates>,
    /// Altitude of this vertiport
    #[prost(float, tag = "3")]
    pub altitude_meters: f32,
    /// Vertiport label
    #[prost(string, optional, tag = "4")]
    pub label: ::core::option::Option<::prost::alloc::string::String>,
    /// Network Timestamp
    #[prost(message, optional, tag = "5")]
    pub timestamp_network: ::core::option::Option<::lib_common::time::Timestamp>,
}
/// Waypoint Type
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Waypoint {
    /// Unique identifier
    #[prost(string, tag = "1")]
    pub identifier: ::prost::alloc::string::String,
    /// Latitude Coordinate
    #[prost(message, optional, tag = "2")]
    pub location: ::core::option::Option<Coordinates>,
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
pub struct Zone {
    /// Unique identifier (NOTAM id, etc.)
    #[prost(string, tag = "1")]
    pub identifier: ::prost::alloc::string::String,
    /// Zone Type
    #[prost(enumeration = "ZoneType", tag = "2")]
    pub zone_type: i32,
    /// Vertices bounding the No-Fly Zone
    /// The first vertex should match the end vertex (closed shape)
    #[prost(message, repeated, tag = "3")]
    pub vertices: ::prost::alloc::vec::Vec<Coordinates>,
    /// Minimum altitude for this zone
    #[prost(float, tag = "4")]
    pub altitude_meters_min: f32,
    /// Maximum altitude for this zone
    #[prost(float, tag = "5")]
    pub altitude_meters_max: f32,
    /// Start datetime for this zone
    #[prost(message, optional, tag = "6")]
    pub time_start: ::core::option::Option<::lib_common::time::Timestamp>,
    /// End datetime for this zone
    #[prost(message, optional, tag = "7")]
    pub time_end: ::core::option::Option<::lib_common::time::Timestamp>,
}
/// Update No Fly Zones Request object
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct UpdateZonesRequest {
    /// Nodes to update
    #[prost(message, repeated, tag = "1")]
    pub zones: ::prost::alloc::vec::Vec<Zone>,
}
/// Update flight paths
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct UpdateFlightPathRequest {
    /// The unique identifier for the flight
    #[prost(string, optional, tag = "1")]
    pub flight_identifier: ::core::option::Option<::prost::alloc::string::String>,
    /// The unique identifier for the aircraft
    #[prost(string, optional, tag = "2")]
    pub aircraft_identifier: ::core::option::Option<::prost::alloc::string::String>,
    /// If this is a simulated flight
    #[prost(bool, tag = "3")]
    pub simulated: bool,
    /// The type of aircraft
    #[prost(enumeration = "crate::prelude::AircraftType", tag = "4")]
    pub aircraft_type: i32,
    /// The path of the aircraft
    #[prost(message, repeated, tag = "5")]
    pub path: ::prost::alloc::vec::Vec<PointZ>,
    /// The planned start time of the flight
    #[prost(message, optional, tag = "6")]
    pub timestamp_start: ::core::option::Option<::lib_common::time::Timestamp>,
    /// The planned end time of the flight
    #[prost(message, optional, tag = "7")]
    pub timestamp_end: ::core::option::Option<::lib_common::time::Timestamp>,
}
/// Best Path Request object
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct BestPathRequest {
    /// Start Node Identifier
    #[prost(string, tag = "1")]
    pub origin_identifier: ::prost::alloc::string::String,
    /// End Node (Vertiport UUID)
    #[prost(string, tag = "2")]
    pub target_identifier: ::prost::alloc::string::String,
    /// Routing Type (Vertiport or Aircraft Allowed)
    #[prost(enumeration = "NodeType", tag = "3")]
    pub origin_type: i32,
    /// Routing Type (Vertiport or Aircraft Allowed)
    #[prost(enumeration = "NodeType", tag = "4")]
    pub target_type: i32,
    /// Time of departure
    #[prost(message, optional, tag = "5")]
    pub time_start: ::core::option::Option<::lib_common::time::Timestamp>,
    /// Time of arrival
    #[prost(message, optional, tag = "6")]
    pub time_end: ::core::option::Option<::lib_common::time::Timestamp>,
    /// Number of paths to return
    #[prost(int32, tag = "7")]
    pub limit: i32,
}
/// Check Intersection Request object
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct CheckIntersectionRequest {
    /// Start Node Identifier
    #[prost(string, tag = "1")]
    pub origin_identifier: ::prost::alloc::string::String,
    /// End Node (Vertiport UUID)
    #[prost(string, tag = "2")]
    pub target_identifier: ::prost::alloc::string::String,
    /// The path to check
    #[prost(message, repeated, tag = "3")]
    pub path: ::prost::alloc::vec::Vec<PointZ>,
    /// Time of departure
    #[prost(message, optional, tag = "4")]
    pub time_start: ::core::option::Option<::lib_common::time::Timestamp>,
    /// Time of arrival
    #[prost(message, optional, tag = "5")]
    pub time_end: ::core::option::Option<::lib_common::time::Timestamp>,
}
/// Check Intersection Response object
#[derive(Eq, Copy)]
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct CheckIntersectionResponse {
    /// True if the path intersects a zone or previous plan
    #[prost(bool, tag = "1")]
    pub intersects: bool,
}
/// / Geospatial Point with Altitude
#[derive(Copy, ::serde::Serialize, ::serde::Deserialize)]
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct PointZ {
    /// Latitude
    #[prost(double, tag = "1")]
    pub latitude: f64,
    /// Longitude
    #[prost(double, tag = "2")]
    pub longitude: f64,
    /// Altitude
    #[prost(float, tag = "3")]
    pub altitude_meters: f32,
}
/// / A node in a path
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct PathNode {
    /// Path Node Index
    #[prost(int32, tag = "1")]
    pub index: i32,
    /// Node Type (Vertiport or Waypoint)
    #[prost(enumeration = "NodeType", tag = "2")]
    pub node_type: i32,
    /// Node Identifier
    #[prost(string, tag = "3")]
    pub identifier: ::prost::alloc::string::String,
    /// Location
    #[prost(message, optional, tag = "4")]
    pub geom: ::core::option::Option<PointZ>,
}
/// / A path between nodes
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Path {
    /// The nodes in this path
    #[prost(message, repeated, tag = "1")]
    pub path: ::prost::alloc::vec::Vec<PathNode>,
    /// Total distance of this path
    #[prost(float, tag = "2")]
    pub distance_meters: f32,
}
/// Best Path Response object
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct BestPathResponse {
    /// Best paths
    #[prost(message, repeated, tag = "1")]
    pub paths: ::prost::alloc::vec::Vec<Path>,
}
/// Get Flights Request object
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GetFlightsRequest {
    /// GPS Rectangular Window Corner Min X
    #[prost(double, tag = "1")]
    pub window_min_x: f64,
    /// GPS Rectangular Window Corner Min Y
    #[prost(double, tag = "2")]
    pub window_min_y: f64,
    /// GPS Rectangular Window Corner Max X
    #[prost(double, tag = "3")]
    pub window_max_x: f64,
    /// GPS Rectangular Window Corner Max Y
    #[prost(double, tag = "4")]
    pub window_max_y: f64,
    /// Time window start
    #[prost(message, optional, tag = "5")]
    pub time_start: ::core::option::Option<::lib_common::time::Timestamp>,
    /// Time window end
    #[prost(message, optional, tag = "6")]
    pub time_end: ::core::option::Option<::lib_common::time::Timestamp>,
}
/// Timestamped position of an aircraft
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct TimePosition {
    /// Aircraft Position
    #[prost(message, optional, tag = "1")]
    pub position: ::core::option::Option<PointZ>,
    /// Timestamp
    #[prost(message, optional, tag = "2")]
    pub timestamp: ::core::option::Option<::lib_common::time::Timestamp>,
}
/// The state of the aircraft including position, status, and velocity
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct AircraftState {
    /// The timestamp of the state
    #[prost(message, optional, tag = "1")]
    pub timestamp: ::core::option::Option<::lib_common::time::Timestamp>,
    /// The operational status of the aircraft
    #[prost(enumeration = "crate::prelude::OperationalStatus", tag = "2")]
    pub status: i32,
    /// The position of the aircraft
    #[prost(message, optional, tag = "3")]
    pub position: ::core::option::Option<PointZ>,
    /// The track angle of the aircraft
    #[prost(float, tag = "4")]
    pub track_angle_degrees: f32,
    /// The ground speed of the aircraft
    #[prost(float, tag = "5")]
    pub ground_speed_mps: f32,
    /// The vertical speed of the aircraft
    #[prost(float, tag = "6")]
    pub vertical_speed_mps: f32,
}
/// Aircraft Flight Information
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Flight {
    /// Flight identifier, if on assigned flight
    #[prost(string, optional, tag = "1")]
    pub session_id: ::core::option::Option<::prost::alloc::string::String>,
    /// Aircraft identifier
    #[prost(string, optional, tag = "2")]
    pub aircraft_id: ::core::option::Option<::prost::alloc::string::String>,
    /// If this is a simulated aircraft
    #[prost(bool, tag = "3")]
    pub simulated: bool,
    /// The timestamped positions of the aircraft
    #[prost(message, repeated, tag = "4")]
    pub positions: ::prost::alloc::vec::Vec<TimePosition>,
    /// The type of aircraft
    #[prost(enumeration = "crate::prelude::AircraftType", tag = "5")]
    pub aircraft_type: i32,
    /// The state of the aircraft
    #[prost(message, optional, tag = "6")]
    pub state: ::core::option::Option<AircraftState>,
}
/// Get Flights Response object
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GetFlightsResponse {
    /// Flights in the requested zone
    #[prost(message, repeated, tag = "1")]
    pub flights: ::prost::alloc::vec::Vec<Flight>,
}
/// The nodes involved in the best path request
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, ::prost::Enumeration)]
#[repr(i32)]
pub enum NodeType {
    /// Vertiport
    Vertiport = 0,
    /// Waypoint
    Waypoint = 1,
    /// Aircraft
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
/// Airspace Zone Type
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, ::prost::Enumeration)]
#[repr(i32)]
pub enum ZoneType {
    /// Vertiport
    Port = 0,
    /// Restriction
    Restriction = 1,
}
impl ZoneType {
    /// String value of the enum field names used in the ProtoBuf definition.
    ///
    /// The values are not transformed in any way and thus are considered stable
    /// (if the ProtoBuf definition does not change) and safe for programmatic use.
    pub fn as_str_name(&self) -> &'static str {
        match self {
            ZoneType::Port => "PORT",
            ZoneType::Restriction => "RESTRICTION",
        }
    }
    /// Creates an enum from field names used in the ProtoBuf definition.
    pub fn from_str_name(value: &str) -> ::core::option::Option<Self> {
        match value {
            "PORT" => Some(Self::Port),
            "RESTRICTION" => Some(Self::Restriction),
            _ => None,
        }
    }
}
/// Generated client implementations.
pub mod rpc_service_client {
    #![allow(unused_variables, dead_code, missing_docs, clippy::let_unit_value)]
    use tonic::codegen::*;
    use tonic::codegen::http::Uri;
    #[derive(Debug, Clone)]
    pub struct RpcServiceClient<T> {
        inner: tonic::client::Grpc<T>,
    }
    impl RpcServiceClient<tonic::transport::Channel> {
        /// Attempt to create a new client by connecting to a given endpoint.
        pub async fn connect<D>(dst: D) -> Result<Self, tonic::transport::Error>
        where
            D: TryInto<tonic::transport::Endpoint>,
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
        /// Limits the maximum size of a decoded message.
        ///
        /// Default: `4MB`
        #[must_use]
        pub fn max_decoding_message_size(mut self, limit: usize) -> Self {
            self.inner = self.inner.max_decoding_message_size(limit);
            self
        }
        /// Limits the maximum size of an encoded message.
        ///
        /// Default: `usize::MAX`
        #[must_use]
        pub fn max_encoding_message_size(mut self, limit: usize) -> Self {
            self.inner = self.inner.max_encoding_message_size(limit);
            self
        }
        pub async fn is_ready(
            &mut self,
            request: impl tonic::IntoRequest<super::ReadyRequest>,
        ) -> std::result::Result<tonic::Response<super::ReadyResponse>, tonic::Status> {
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
            let mut req = request.into_request();
            req.extensions_mut().insert(GrpcMethod::new("grpc.RpcService", "isReady"));
            self.inner.unary(req, path, codec).await
        }
        pub async fn update_vertiports(
            &mut self,
            request: impl tonic::IntoRequest<super::UpdateVertiportsRequest>,
        ) -> std::result::Result<tonic::Response<super::UpdateResponse>, tonic::Status> {
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
            let mut req = request.into_request();
            req.extensions_mut()
                .insert(GrpcMethod::new("grpc.RpcService", "updateVertiports"));
            self.inner.unary(req, path, codec).await
        }
        pub async fn update_waypoints(
            &mut self,
            request: impl tonic::IntoRequest<super::UpdateWaypointsRequest>,
        ) -> std::result::Result<tonic::Response<super::UpdateResponse>, tonic::Status> {
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
            let mut req = request.into_request();
            req.extensions_mut()
                .insert(GrpcMethod::new("grpc.RpcService", "updateWaypoints"));
            self.inner.unary(req, path, codec).await
        }
        pub async fn update_zones(
            &mut self,
            request: impl tonic::IntoRequest<super::UpdateZonesRequest>,
        ) -> std::result::Result<tonic::Response<super::UpdateResponse>, tonic::Status> {
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
                "/grpc.RpcService/updateZones",
            );
            let mut req = request.into_request();
            req.extensions_mut()
                .insert(GrpcMethod::new("grpc.RpcService", "updateZones"));
            self.inner.unary(req, path, codec).await
        }
        pub async fn update_flight_path(
            &mut self,
            request: impl tonic::IntoRequest<super::UpdateFlightPathRequest>,
        ) -> std::result::Result<tonic::Response<super::UpdateResponse>, tonic::Status> {
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
                "/grpc.RpcService/updateFlightPath",
            );
            let mut req = request.into_request();
            req.extensions_mut()
                .insert(GrpcMethod::new("grpc.RpcService", "updateFlightPath"));
            self.inner.unary(req, path, codec).await
        }
        pub async fn best_path(
            &mut self,
            request: impl tonic::IntoRequest<super::BestPathRequest>,
        ) -> std::result::Result<
            tonic::Response<super::BestPathResponse>,
            tonic::Status,
        > {
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
            let mut req = request.into_request();
            req.extensions_mut().insert(GrpcMethod::new("grpc.RpcService", "bestPath"));
            self.inner.unary(req, path, codec).await
        }
        pub async fn check_intersection(
            &mut self,
            request: impl tonic::IntoRequest<super::CheckIntersectionRequest>,
        ) -> std::result::Result<
            tonic::Response<super::CheckIntersectionResponse>,
            tonic::Status,
        > {
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
                "/grpc.RpcService/checkIntersection",
            );
            let mut req = request.into_request();
            req.extensions_mut()
                .insert(GrpcMethod::new("grpc.RpcService", "checkIntersection"));
            self.inner.unary(req, path, codec).await
        }
        pub async fn get_flights(
            &mut self,
            request: impl tonic::IntoRequest<super::GetFlightsRequest>,
        ) -> std::result::Result<
            tonic::Response<super::GetFlightsResponse>,
            tonic::Status,
        > {
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
                "/grpc.RpcService/getFlights",
            );
            let mut req = request.into_request();
            req.extensions_mut()
                .insert(GrpcMethod::new("grpc.RpcService", "getFlights"));
            self.inner.unary(req, path, codec).await
        }
    }
}
