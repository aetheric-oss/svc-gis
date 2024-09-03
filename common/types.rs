use serde::{Deserialize, Serialize};
use lib_common::time::{DateTime, Utc};

/// The key for the Redis queue containing aircraft identification information
pub const REDIS_KEY_AIRCRAFT_ID: &str = "gis:aircraft:id";

/// The key for the Redis queue containing aircraft position information
pub const REDIS_KEY_AIRCRAFT_POSITION: &str = "gis:aircraft:position";

/// The key for the Redis queue containing aircraft velocity information
pub const REDIS_KEY_AIRCRAFT_VELOCITY: &str = "gis:aircraft:velocity";

/// Aircraft Type
#[derive(Serialize, Deserialize, Debug, Copy, Clone, PartialEq)]
#[derive(strum::EnumString)]
#[derive(strum::Display)]
#[derive(strum::EnumIter)]
#[derive(postgres_types::FromSql)]
#[derive(postgres_types::ToSql)]
#[derive(num_derive::FromPrimitive)]
#[postgres(name = "aircrafttype")]
#[derive(::prost::Enumeration)]
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

/// Operational Status
#[derive(Serialize, Deserialize, Debug, Copy, Clone)]
#[derive(strum::EnumString)]
#[derive(strum::Display)]
#[derive(strum::EnumIter)]
#[derive(postgres_types::FromSql)]
#[derive(postgres_types::ToSql)]
#[derive(num_derive::FromPrimitive)]
#[postgres(name = "opstatus")]
#[derive(::prost::Enumeration)]
#[repr(i32)]
pub enum OperationalStatus {
    /// Undeclared status
    Undeclared = 0,

    /// Ground
    Ground = 1,

    /// Airborne
    Airborne = 2,

    /// Emergency
    Emergency = 3,

    /// RemoteID System Failure
    RemoteIdSystemFailure = 4,
}

/// 3D Point with Altitude
#[derive(Serialize, Deserialize, Debug, Copy, Clone)]
pub struct Position {
    /// Longitude in degrees
    pub longitude: f64,

    /// Latitude in degrees
    pub latitude: f64,

    /// Altitude in meters
    pub altitude_meters: f64,
}

/// Generic Location Information for an Aircraft
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct AircraftPosition {
    /// The unique identifier for the aircraft
    pub identifier: String,

    /// The 3D position of the aircraft
    pub position: Position,

    /// The network timestamp of the position
    pub timestamp_network: DateTime<Utc>,

    /// The timestamp reported by the asset
    pub timestamp_asset: Option<DateTime<Utc>>,

    // TODO(R5): location uncertainty
}

/// Generic Identification Information for an Aircraft
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct AircraftId {
    /// The unique identifier for the aircraft
    pub identifier: Option<String>,

    /// The flight ID of this aircraft
    pub session_id: Option<String>,

    /// The type of aircraft
    pub aircraft_type: AircraftType,

    /// The network timestamp of the identification
    pub timestamp_network: DateTime<Utc>,

    /// The timestamp reported by the asset
    pub timestamp_asset: Option<DateTime<Utc>>
}

/// Generic Velocity Information for an Aircraft
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct AircraftVelocity {
    /// The unique identifier for the aircraft
    /// Could be a CAA-assigned ID, a serial number, etc.
    pub identifier: String,

    /// The velocity of the aircraft relative to ground in meters per second
    ///  If the aircraft has a headwind of 100 kph and is not moving
    ///  with respect to ground, its ground speed is 0 but its airspeed is 100 kph.
    pub velocity_horizontal_ground_mps: f32,

    /// The velocity of the aircraft relative to the air in meters per second
    pub velocity_horizontal_air_mps: Option<f32>,

    /// The vertical velocity of the aircraft in meters per second
    pub velocity_vertical_mps: f32,

    /// The angle of the velocity vector with respect to true north in degrees
    pub track_angle_degrees: f32,

    /// The network timestamp of the velocity
    pub timestamp_network: DateTime<Utc>,
    
    /// The timestamp reported by the asset
    pub timestamp_asset: Option<DateTime<Utc>>

    // TODO(R5): velocity uncertainty
}
