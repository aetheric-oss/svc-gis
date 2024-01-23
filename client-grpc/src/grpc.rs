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
/// Aircraft Identification
/// ADS-B issues an identification message separately from position
/// at a different rate, indicating the type of aircraft.
/// Remote ID uses basic ID
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct AircraftId {
    /// Aircraft Identifier
    #[prost(string, tag = "1")]
    pub identifier: ::prost::alloc::string::String,
    /// Aircraft Type
    #[prost(enumeration = "AircraftType", tag = "2")]
    pub aircraft_type: i32,
    /// Network Timestamp at Receipt
    #[prost(message, optional, tag = "3")]
    pub timestamp_network: ::core::option::Option<::lib_common::time::Timestamp>,
}
/// Aircraft Type
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct AircraftPosition {
    /// Aircraft Identifier
    #[prost(string, tag = "1")]
    pub identifier: ::prost::alloc::string::String,
    /// Aircraft Location
    #[prost(message, optional, tag = "2")]
    pub geom: ::core::option::Option<PointZ>,
    /// Telemetry Self-Report Time
    #[prost(message, optional, tag = "3")]
    pub timestamp_aircraft: ::core::option::Option<::lib_common::time::Timestamp>,
    /// Network Timestamp at Receipt
    #[prost(message, optional, tag = "4")]
    pub timestamp_network: ::core::option::Option<::lib_common::time::Timestamp>,
}
/// Aircraft Identification
/// ADS-B issues an identification message separately from position
/// at a different rate, indicating the type of aircraft.
/// Remote ID includes velocity in location messages
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct AircraftVelocity {
    /// Aircraft Identifier
    #[prost(string, tag = "1")]
    pub identifier: ::prost::alloc::string::String,
    /// Vertical Rate (meters/second)
    #[prost(float, tag = "2")]
    pub velocity_vertical_mps: f32,
    /// Horizontal Rate (Ground, meters/second)
    /// ADS-B and Remote ID Location message both report this
    #[prost(float, tag = "3")]
    pub velocity_horizontal_ground_mps: f32,
    /// Horizontal Rate (Air, meters/second)
    /// ADS-B may report this, Remote ID does not
    #[prost(float, optional, tag = "4")]
    pub velocity_horizontal_air_mps: ::core::option::Option<f32>,
    /// Track Angle (degrees) from true North
    #[prost(float, tag = "5")]
    pub track_angle_degrees: f32,
    /// Telemetry Self-Report Time
    #[prost(message, optional, tag = "6")]
    pub timestamp_aircraft: ::core::option::Option<::lib_common::time::Timestamp>,
    /// Network Timestamp at Receipt
    #[prost(message, optional, tag = "7")]
    pub timestamp_network: ::core::option::Option<::lib_common::time::Timestamp>,
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
/// Update Aircraft Request Object
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct UpdateAircraftIdRequest {
    /// List of aircraft to update
    #[prost(message, repeated, tag = "1")]
    pub aircraft: ::prost::alloc::vec::Vec<AircraftId>,
}
/// Update Aircraft Request Object
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct UpdateAircraftPositionRequest {
    /// List of aircraft to update
    #[prost(message, repeated, tag = "1")]
    pub aircraft: ::prost::alloc::vec::Vec<AircraftPosition>,
}
/// Update Aircraft Request Object
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct UpdateAircraftVelocityRequest {
    /// List of aircraft to update
    #[prost(message, repeated, tag = "1")]
    pub aircraft: ::prost::alloc::vec::Vec<AircraftVelocity>,
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
/// / Geospatial Point with Altitude
#[derive(Copy)]
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
/// Aircraft Type
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, ::prost::Enumeration)]
#[repr(i32)]
pub enum AircraftType {
    /// Undeclared aircraft type
    Undeclared = 0,
    /// Fixed Wing Aircraft
    Aeroplane = 1,
    /// Rotary Wing Aircraft
    Rotorcraft = 2,
    /// Gyroplane
    Gyroplane = 3,
    /// Hybrid Lift
    Hybridlift = 4,
    /// Ornithopter
    Ornithopter = 5,
    /// Glider
    Glider = 6,
    /// Kite
    Kite = 7,
    /// Free Balloon
    Freeballoon = 8,
    /// Captive Balloon
    Captiveballoon = 9,
    /// Airship
    Airship = 10,
    /// Unpowered aircraft (free fall or parachute)
    Unpowered = 11,
    /// Rocket
    Rocket = 12,
    /// Tethered Powered Aircraft
    Tethered = 13,
    /// Ground Obstacle
    Groundobstacle = 14,
    /// Other
    Other = 15,
}
impl AircraftType {
    /// String value of the enum field names used in the ProtoBuf definition.
    ///
    /// The values are not transformed in any way and thus are considered stable
    /// (if the ProtoBuf definition does not change) and safe for programmatic use.
    pub fn as_str_name(&self) -> &'static str {
        match self {
            AircraftType::Undeclared => "UNDECLARED",
            AircraftType::Aeroplane => "AEROPLANE",
            AircraftType::Rotorcraft => "ROTORCRAFT",
            AircraftType::Gyroplane => "GYROPLANE",
            AircraftType::Hybridlift => "HYBRIDLIFT",
            AircraftType::Ornithopter => "ORNITHOPTER",
            AircraftType::Glider => "GLIDER",
            AircraftType::Kite => "KITE",
            AircraftType::Freeballoon => "FREEBALLOON",
            AircraftType::Captiveballoon => "CAPTIVEBALLOON",
            AircraftType::Airship => "AIRSHIP",
            AircraftType::Unpowered => "UNPOWERED",
            AircraftType::Rocket => "ROCKET",
            AircraftType::Tethered => "TETHERED",
            AircraftType::Groundobstacle => "GROUNDOBSTACLE",
            AircraftType::Other => "OTHER",
        }
    }
    /// Creates an enum from field names used in the ProtoBuf definition.
    pub fn from_str_name(value: &str) -> ::core::option::Option<Self> {
        match value {
            "UNDECLARED" => Some(Self::Undeclared),
            "AEROPLANE" => Some(Self::Aeroplane),
            "ROTORCRAFT" => Some(Self::Rotorcraft),
            "GYROPLANE" => Some(Self::Gyroplane),
            "HYBRIDLIFT" => Some(Self::Hybridlift),
            "ORNITHOPTER" => Some(Self::Ornithopter),
            "GLIDER" => Some(Self::Glider),
            "KITE" => Some(Self::Kite),
            "FREEBALLOON" => Some(Self::Freeballoon),
            "CAPTIVEBALLOON" => Some(Self::Captiveballoon),
            "AIRSHIP" => Some(Self::Airship),
            "UNPOWERED" => Some(Self::Unpowered),
            "ROCKET" => Some(Self::Rocket),
            "TETHERED" => Some(Self::Tethered),
            "GROUNDOBSTACLE" => Some(Self::Groundobstacle),
            "OTHER" => Some(Self::Other),
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
    /// Heartbeat
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
        /// Common Interfaces
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
        pub async fn update_aircraft_id(
            &mut self,
            request: impl tonic::IntoRequest<super::UpdateAircraftIdRequest>,
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
                "/grpc.RpcService/updateAircraftId",
            );
            let mut req = request.into_request();
            req.extensions_mut()
                .insert(GrpcMethod::new("grpc.RpcService", "updateAircraftId"));
            self.inner.unary(req, path, codec).await
        }
        pub async fn update_aircraft_position(
            &mut self,
            request: impl tonic::IntoRequest<super::UpdateAircraftPositionRequest>,
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
                "/grpc.RpcService/updateAircraftPosition",
            );
            let mut req = request.into_request();
            req.extensions_mut()
                .insert(GrpcMethod::new("grpc.RpcService", "updateAircraftPosition"));
            self.inner.unary(req, path, codec).await
        }
        pub async fn update_aircraft_velocity(
            &mut self,
            request: impl tonic::IntoRequest<super::UpdateAircraftVelocityRequest>,
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
                "/grpc.RpcService/updateAircraftVelocity",
            );
            let mut req = request.into_request();
            req.extensions_mut()
                .insert(GrpcMethod::new("grpc.RpcService", "updateAircraftVelocity"));
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
    }
}
