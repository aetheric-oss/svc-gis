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

use crate::grpc::server::NodeType as GrpcNodeType;
use postgres_types::FromSql;

/// Routing can occur from a vertiport to a vertiport
/// Or an aircraft to a vertiport (in-flight re-routing)
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum PathType {
    /// Route between vertiports
    PortToPort = 0,

    /// Route from an aircraft to a vertiport
    AircraftToPort = 1,
}

/// Types of nodes returned by the routing algorithm
#[derive(Debug, Copy, Clone, FromSql, PartialEq)]
#[postgres(name = "nodetype")]
pub enum NodeType {
    /// Vertiport Node
    #[postgres(name = "vertiport")]
    Vertiport,

    /// Waypoint Node
    #[postgres(name = "waypoint")]
    Waypoint,

    /// Aircraft Node
    #[postgres(name = "aircraft")]
    Aircraft,
}

impl TryFrom<i32> for NodeType {
    type Error = ();
    fn try_from(v: i32) -> Result<Self, Self::Error> {
        match v {
            x if x == NodeType::Vertiport as i32 => Ok(NodeType::Vertiport),
            x if x == NodeType::Aircraft as i32 => Ok(NodeType::Aircraft),
            x if x == NodeType::Waypoint as i32 => Ok(NodeType::Waypoint),
            _ => Err(()),
        }
    }
}

impl TryFrom<i32> for GrpcNodeType {
    type Error = ();
    fn try_from(v: i32) -> Result<Self, Self::Error> {
        match v {
            x if x == GrpcNodeType::Vertiport as i32 => Ok(GrpcNodeType::Vertiport),
            x if x == GrpcNodeType::Aircraft as i32 => Ok(GrpcNodeType::Aircraft),
            x if x == GrpcNodeType::Waypoint as i32 => Ok(GrpcNodeType::Waypoint),
            _ => Err(()),
        }
    }
}

impl From<NodeType> for GrpcNodeType {
    fn from(node_type: NodeType) -> Self {
        match node_type {
            NodeType::Vertiport => GrpcNodeType::Vertiport,
            NodeType::Waypoint => GrpcNodeType::Waypoint,
            NodeType::Aircraft => GrpcNodeType::Aircraft,
        }
    }
}

impl From<GrpcNodeType> for NodeType {
    fn from(node_type: GrpcNodeType) -> Self {
        match node_type {
            GrpcNodeType::Vertiport => NodeType::Vertiport,
            GrpcNodeType::Waypoint => NodeType::Waypoint,
            GrpcNodeType::Aircraft => NodeType::Aircraft,
        }
    }
}
