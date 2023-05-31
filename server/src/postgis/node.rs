//! This module contains functions for updating nodes in the PostGIS database.
//! Nodes are columns in space that aircraft can fly between.
//! Nodes can include fixed aviation coordinates (waypoints) or vertiports.

#[derive(Debug, Clone, Copy)]
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

#[derive(Debug, Copy, Clone)]
/// Nodes that aircraft can fly between
pub struct Node {
    /// The UUID of the node
    pub uuid: uuid::Uuid,

    /// The latitude of the node
    pub latitude: f64,

    /// The longitude of the node
    pub longitude: f64,

    /// The type of node (vertiport, waypoint, etc.)
    pub node_type: NodeType,
}

/// Updates nodes in the PostGIS database.
pub async fn update_nodes(nodes: Vec<Node>, pool: deadpool_postgres::Pool) -> Result<(), ()> {
    postgis_debug!("(postgis update_node) entry.");

    // TODO(R4): prepared statement
    for node in nodes {
        // In SRID 4326, Point(X Y) is (longitude latitude)
        let cmd_str = format!(
            "
        INSERT INTO arrow.rnodes (arrow_id, node_type, geom)
            VALUES ('{}', '{}', 'SRID=4326;POINT({} {})')
            ON CONFLICT(arrow_id)
                DO UPDATE
                    SET geom = EXCLUDED.geom;",
            node.uuid, node.node_type, node.longitude, node.latitude
        );

        match super::execute_psql_cmd(cmd_str, pool.clone()).await {
            Ok(_) => (),
            Err(e) => {
                println!("(postgis update_nodes) Error executing command: {:?}", e);
                return Err(());
            }
        }
    }

    Ok(())
}
