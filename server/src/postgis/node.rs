//! This module contains functions for updating nodes in the PostGIS database.
//! Nodes are columns in space that aircraft can fly between.
//! Nodes can include fixed aviation coordinates (waypoints) or vertiports.

use crate::grpc::server::grpc_server;
use crate::postgis::node::Node as GisNode;
use crate::postgis::node::NodeType as GisNodeType;
use grpc_server::Node as RequestNode;

#[derive(Debug, Clone, Copy, PartialEq)]
/// The type of node (vertiport, waypoint, etc.)
pub enum NodeType {
    /// Waypoints are nodes that aircraft can fly between
    Waypoint = 0,

    /// Vertiports are nodes that mark the start and end of journeys
    Vertiport = 1,
}

impl std::fmt::Display for NodeType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            NodeType::Waypoint => write!(f, "waypoint"),
            NodeType::Vertiport => write!(f, "vertiport"),
        }
    }
}

/// Possible conversion errors from the GRPC type to GIS type
#[derive(Debug, Copy, Clone, PartialEq)]
pub enum NodeError {
    /// Invalid UUID
    BadUuid,

    /// Invalid Node Type
    UnrecognizedType,

    /// No location provided
    NoLocation,

    /// No Nodes
    NoNodes,
}

impl std::fmt::Display for NodeError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            NodeError::UnrecognizedType => write!(f, "Invalid node type provided."),
            NodeError::BadUuid => write!(f, "Invalid node UUID provided."),
            NodeError::NoLocation => write!(f, "No location was provided."),
            NodeError::NoNodes => write!(f, "No nodes were provided."),
        }
    }
}

#[derive(Debug, Copy, Clone)]
/// Nodes that aircraft can fly between
pub struct Node {
    /// The UUID of the node
    pub uuid: uuid::Uuid,

    /// The latitude of the node
    pub latitude: f32,

    /// The longitude of the node
    pub longitude: f32,

    /// The type of node (vertiport, waypoint, etc.)
    pub node_type: NodeType,
}

/// Convert nodes from the GRPC request into nodes for the GIS database,
///  detecting invalid arguments and returning an error if necessary.
pub fn nodes_grpc_to_gis(req_nodes: Vec<RequestNode>) -> Result<Vec<GisNode>, NodeError> {
    if req_nodes.is_empty() {
        return Err(NodeError::NoNodes);
    }

    let mut nodes: Vec<GisNode> = vec![];
    for node in &req_nodes {
        let uuid = match uuid::Uuid::parse_str(&node.uuid) {
            Ok(uuid) => uuid,
            Err(e) => {
                postgis_error!("(nodes_grpc_to_gis) failed to parse uuid: {}", e);
                return Err(NodeError::BadUuid);
            }
        };

        let node_type = match node.node_type {
            x if x == (grpc_server::NodeType::Vertiport as i32) => GisNodeType::Vertiport,
            y if y == (grpc_server::NodeType::Waypoint as i32) => GisNodeType::Waypoint,
            e => {
                postgis_error!("(update_node) invalid node type: {}", e);
                return Err(NodeError::UnrecognizedType);
            }
        };

        let (latitude, longitude) = match &node.location {
            Some(l) => (l.latitude, l.longitude),
            _ => {
                postgis_error!("(update_node) no location provided for node.");
                return Err(NodeError::NoLocation);
            }
        };

        // TODO(R4): Check if lat, lon inside geofence for this region
        let node = GisNode {
            uuid,
            latitude,
            longitude,
            node_type,
        };

        nodes.push(node);
    }

    Ok(nodes)
}

/// Updates nodes in the PostGIS database.
pub async fn update_nodes(nodes: Vec<Node>, pool: deadpool_postgres::Pool) -> Result<(), ()> {
    postgis_debug!("(postgis update_node) entry.");

    // TODO(R4): prepared statement
    for node in &nodes {
        // In SRID 4326, Point(X Y) is (longitude latitude)
        let cmd_str = format!(
            "
        INSERT INTO arrow.rnodes (arrow_id, node_type, geom)
            VALUES ('{}'::UUID, '{}', 'SRID=4326;POINT({} {})')
            ON CONFLICT(arrow_id)
                DO UPDATE
                    SET geom = EXCLUDED.geom;",
            node.uuid, node.node_type, node.longitude, node.latitude
        );

        match super::execute_psql_cmd(cmd_str, pool.clone()).await {
            Ok(_) => (),
            Err(e) => {
                postgis_error!("(postgis update_nodes) Error executing command: {:?}", e);
                return Err(());
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::grpc::server::grpc_server::Coordinates;

    #[test]
    fn ut_nodes_request_to_gis_valid() {
        let request_nodes: Vec<RequestNode> = vec![
            RequestNode {
                uuid: uuid::Uuid::new_v4().to_string(),
                location: Some(Coordinates {
                    latitude: 0.0,
                    longitude: 0.0,
                }),
                node_type: grpc_server::NodeType::Vertiport as i32,
            },
            RequestNode {
                uuid: uuid::Uuid::new_v4().to_string(),
                location: Some(Coordinates {
                    latitude: 1.0,
                    longitude: 1.0,
                }),
                node_type: grpc_server::NodeType::Waypoint as i32,
            },
        ];

        let nodes = match nodes_grpc_to_gis(request_nodes.clone()) {
            Ok(nodes) => nodes,
            Err(_) => panic!("Failed to convert nodes."),
        };

        assert_eq!(nodes.len(), request_nodes.len());

        for (i, node) in nodes.iter().enumerate() {
            let Some(location) = request_nodes[i].location else {
                panic!();
            };

            assert_eq!(node.uuid.to_string(), request_nodes[i].uuid);
            assert_eq!(node.latitude, location.latitude);
            assert_eq!(node.longitude, location.longitude);
        }
    }

    #[test]
    fn ut_nodes_request_to_gis_invalid_uuid() {
        let request_nodes: Vec<RequestNode> = vec![RequestNode {
            uuid: "invalid".to_string(),
            location: Some(Coordinates {
                latitude: 0.0,
                longitude: 0.0,
            }),
            node_type: grpc_server::NodeType::Vertiport as i32,
        }];

        let result = nodes_grpc_to_gis(request_nodes).unwrap_err();
        assert_eq!(result, NodeError::BadUuid);
    }

    #[test]
    fn ut_nodes_request_to_gis_invalid_no_nodes() {
        let request_nodes: Vec<RequestNode> = vec![];
        let result = nodes_grpc_to_gis(request_nodes).unwrap_err();
        assert_eq!(result, NodeError::NoNodes);
    }

    #[test]
    fn ut_nodes_request_to_gis_invalid_node_type() {
        let request_nodes: Vec<RequestNode> = vec![RequestNode {
            uuid: uuid::Uuid::new_v4().to_string(),
            location: Some(Coordinates {
                latitude: 0.0,
                longitude: 0.0,
            }),
            node_type: grpc_server::NodeType::Vertiport as i32 + 1,
        }];

        let result = nodes_grpc_to_gis(request_nodes).unwrap_err();
        assert_eq!(result, NodeError::UnrecognizedType);
    }

    #[test]
    fn ut_nodes_request_to_gis_invalid_location() {
        let request_nodes: Vec<RequestNode> = vec![RequestNode {
            uuid: uuid::Uuid::new_v4().to_string(),
            location: None,
            node_type: grpc_server::NodeType::Vertiport as i32,
        }];

        let result = nodes_grpc_to_gis(request_nodes).unwrap_err();
        assert_eq!(result, NodeError::NoLocation);
    }
}
