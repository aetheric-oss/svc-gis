#![doc = include_str!("./README.md")]

#[macro_use]
pub mod macros;
pub mod aircraft;
pub mod best_path;
pub mod nearest;
pub mod nofly;
pub mod pool;
pub mod utils;
pub mod vertiport;
pub mod waypoint;

/// Routing can occur from a vertiport to a vertiport
/// Or an aircraft to a vertiport (in-flight re-routing)
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum PathType {
    /// Route between vertiports
    PortToPort = 0,

    /// Route from an aircraft to a vertiport
    AircraftToPort = 1,
}
