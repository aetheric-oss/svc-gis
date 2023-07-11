#![doc = include_str!("./README.md")]

#[macro_use]
pub mod macros;
pub mod aircraft;
pub mod nofly;
pub mod pool;
pub mod routing;
pub mod utils;
pub mod vertiport;
pub mod waypoint;

use postgres_types::FromSql;

/// Types of nodes returned by the routing algorithm
#[derive(Debug, Copy, Clone, FromSql)]
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

impl From<NodeType> for crate::grpc::server::NodeType {
    fn from(node_type: NodeType) -> Self {
        match node_type {
            NodeType::Vertiport => crate::grpc::server::NodeType::Vertiport,
            NodeType::Waypoint => crate::grpc::server::NodeType::Waypoint,
            NodeType::Aircraft => crate::grpc::server::NodeType::Aircraft,
        }
    }
}
