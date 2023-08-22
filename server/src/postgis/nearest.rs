//! This module contains functions for routing between nodes.
use crate::grpc::server::grpc_server::{
    DistanceTo, NearestNeighborRequest, NodeType as GrpcNodeType,
};

use crate::postgis::NodeType;
use uuid::Uuid;

/// Possible errors with path requests
#[derive(Debug, Copy, Clone, PartialEq)]
pub enum NNError {
    /// No path was found
    NoPath,

    /// Invalid start node
    InvalidStartNode,

    /// Invalid end node
    InvalidEndNode,

    /// Invalid limit
    InvalidLimit,

    /// Invalid range
    InvalidRange,

    /// Unsupported path type
    Unsupported,

    /// Could not get client
    Client,

    /// Unknown error
    Unknown,
}

impl std::fmt::Display for NNError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            NNError::NoPath => write!(f, "No path was found."),
            NNError::InvalidStartNode => write!(f, "Invalid start node."),
            NNError::InvalidEndNode => write!(f, "Invalid end node."),
            NNError::InvalidLimit => write!(f, "Invalid limit."),
            NNError::InvalidRange => write!(f, "Invalid range."),
            NNError::Unsupported => write!(f, "Unsupported path type."),
            NNError::Client => write!(f, "Could not get client."),
            NNError::Unknown => write!(f, "Unknown error."),
        }
    }
}

#[derive(Debug)]
struct NNRequest {
    node_start_id: String,
    start_type: NodeType,
    end_type: NodeType,
    limit: i32,
    max_range_meters: f64,
}

/// Sanitize the request inputs
fn sanitize(request: NearestNeighborRequest) -> Result<NNRequest, NNError> {
    let Ok(start_type) = NodeType::try_from(request.start_type) else {
        postgis_error!(
            "(sanitize) invalid start node type: {:?}",
            request.start_type
        );

        return Err(NNError::InvalidStartNode);
    };

    let Ok(end_type) = NodeType::try_from(request.end_type) else {
        postgis_error!("(sanitize) invalid end node type: {:?}", request.end_type);
        return Err(NNError::InvalidEndNode);
    };

    match start_type {
        NodeType::Vertiport => {
            uuid::Uuid::parse_str(&request.start_node_id).map_err(|_| NNError::InvalidStartNode)?;
        }
        NodeType::Aircraft => {
            crate::postgis::aircraft::check_callsign(&request.start_node_id)
                .map_err(|_| NNError::InvalidStartNode)?;
        }
        _ => {
            postgis_error!("(sanitize) invalid start node type: {:?}", start_type);
            return Err(NNError::Unsupported);
        }
    }

    if end_type != NodeType::Vertiport {
        postgis_error!("(sanitize) invalid end node type: {:?}", end_type);
        return Err(NNError::Unsupported);
    }

    if request.limit < 1 {
        postgis_error!("(sanitize) invalid limit: {}", request.limit);
        return Err(NNError::InvalidLimit);
    }

    if request.max_range_meters < 0.0 {
        postgis_error!(
            "(sanitize) invalid max range meters: {}",
            request.max_range_meters
        );
        return Err(NNError::InvalidRange);
    }

    let node_start_id = request.start_node_id;
    Ok(NNRequest {
        node_start_id,
        start_type,
        end_type,
        limit: request.limit,
        max_range_meters: request.max_range_meters as f64,
    })
}

/// Get the best path from a vertiport to another vertiport
async fn nearest_neighbor_vertiport_source(
    stmt: tokio_postgres::Statement,
    client: deadpool_postgres::Client,
    request: NNRequest,
) -> Result<Vec<tokio_postgres::Row>, NNError> {
    let Ok(node_start_id) = Uuid::parse_str(&request.node_start_id) else {
        postgis_error!("(nearest_neighbor_vertiport_source) could not parse start node id into UUID: {}", request.node_start_id);
        return Err(NNError::InvalidStartNode);
    };

    match client
        .query(
            &stmt,
            &[&node_start_id, &request.limit, &request.max_range_meters],
        )
        .await
    {
        Ok(results) => Ok(results),
        Err(e) => {
            println!(
                "(nearest_neighbor_vertiport_source) could not request routes: {}",
                e
            );
            return Err(NNError::Unknown);
        }
    }
}

/// Get the best path from a aircraft to another vertiport
async fn nearest_neighbor_aircraft_source(
    stmt: tokio_postgres::Statement,
    client: deadpool_postgres::Client,
    request: NNRequest,
) -> Result<Vec<tokio_postgres::Row>, NNError> {
    match client
        .query(
            &stmt,
            &[
                &request.node_start_id,
                &request.limit,
                &request.max_range_meters,
            ],
        )
        .await
    {
        Ok(results) => Ok(results),
        Err(e) => {
            println!(
                "(nearest_neighbor_aircraft_source) could not request routes: {}",
                e
            );
            return Err(NNError::Unknown);
        }
    }
}

/// Nearest neighbor query for nodes
#[cfg(not(tarpaulin_include))]
pub async fn nearest_neighbors(
    request: NearestNeighborRequest,
    pool: deadpool_postgres::Pool,
) -> Result<Vec<DistanceTo>, NNError> {
    let request = sanitize(request)?;

    let Ok(client) = pool.get().await else {
        println!("(nearest_neighbors) could not get client from pool.");
        return Err(NNError::Client);
    };

    let rows = match (request.start_type, request.end_type) {
        (NodeType::Vertiport, NodeType::Vertiport) => {
            let Ok(stmt) = client.prepare_cached(&format!(
                "SELECT * FROM arrow.nearest_vertiports_to_vertiport($1, $2, $3);",
            )).await else {
                postgis_error!("(nearest_neighbors) could not prepare statement.");
                return Err(NNError::Unknown);
            };

            nearest_neighbor_vertiport_source(stmt, client, request).await?
        }
        (NodeType::Aircraft, NodeType::Vertiport) => {
            let Ok(stmt) = client.prepare_cached(&format!(
                "SELECT * FROM arrow.nearest_vertiports_to_aircraft($1, $2, $3);",
            )).await else {
                postgis_error!("(nearest_neighbors) could not prepare statement.");
                return Err(NNError::Unknown);
            };

            nearest_neighbor_aircraft_source(stmt, client, request).await?
        }
        _ => {
            postgis_error!("(grpc nearest_neighbors) unsupported path type");
            return Err(NNError::Unsupported);
        }
    };

    let mut results: Vec<DistanceTo> = vec![];
    for r in &rows {
        let label: Uuid = r.get(0);
        let distance_meters: f64 = r.get(1);

        results.push(DistanceTo {
            label: label.to_string(),
            target_type: GrpcNodeType::Vertiport as i32,
            distance_meters: distance_meters as f32,
        });
    }

    Ok(results)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::grpc::server::grpc_server;

    #[test]
    fn ut_sanitize_valid() {
        let request = NearestNeighborRequest {
            start_node_id: uuid::Uuid::new_v4().to_string(),
            start_type: grpc_server::NodeType::Vertiport as i32,
            end_type: grpc_server::NodeType::Vertiport as i32,
            limit: 10,
            max_range_meters: 1000.0,
        };

        let result = sanitize(request);
        assert!(result.is_ok());
    }

    #[test]
    fn ut_sanitize_valid_aircraft_start() {
        let request = NearestNeighborRequest {
            start_node_id: "test".to_string(),
            start_type: grpc_server::NodeType::Aircraft as i32,
            end_type: grpc_server::NodeType::Vertiport as i32,
            limit: 10,
            max_range_meters: 1000.0,
        };

        let result = sanitize(request);
        assert!(result.is_ok());
    }

    #[test]
    fn ut_sanitize_invalid_uuids() {
        let request = NearestNeighborRequest {
            start_node_id: "Invalid".to_string(),
            start_type: grpc_server::NodeType::Vertiport as i32,
            end_type: grpc_server::NodeType::Vertiport as i32,
            limit: 10,
            max_range_meters: 1000.0,
        };

        let result = sanitize(request).unwrap_err();
        assert_eq!(result, NNError::InvalidStartNode);
    }

    #[test]
    fn ut_sanitize_invalid_aircraft() {
        let request = NearestNeighborRequest {
            start_node_id: "Test-123!".to_string(),
            start_type: grpc_server::NodeType::Aircraft as i32,
            end_type: grpc_server::NodeType::Vertiport as i32,
            limit: 10,
            max_range_meters: 1000.0,
        };

        let result = sanitize(request).unwrap_err();
        assert_eq!(result, NNError::InvalidStartNode);
    }

    #[test]
    fn ut_sanitize_invalid_start_node() {
        let request = NearestNeighborRequest {
            start_node_id: "Aircraft".to_string(),
            start_type: grpc_server::NodeType::Waypoint as i32,
            end_type: grpc_server::NodeType::Vertiport as i32,
            limit: 10,
            max_range_meters: 1000.0,
        };

        let result = sanitize(request).unwrap_err();
        assert_eq!(result, NNError::Unsupported);
    }

    #[test]
    fn ut_sanitize_invalid_end_node() {
        let request = NearestNeighborRequest {
            start_node_id: uuid::Uuid::new_v4().to_string(),
            start_type: grpc_server::NodeType::Vertiport as i32,
            end_type: grpc_server::NodeType::Waypoint as i32,
            limit: 10,
            max_range_meters: 1000.0,
        };

        let result = sanitize(request).unwrap_err();
        assert_eq!(result, NNError::Unsupported);
    }

    #[test]
    fn ut_sanitize_invalid_limit() {
        let request = NearestNeighborRequest {
            start_node_id: uuid::Uuid::new_v4().to_string(),
            start_type: grpc_server::NodeType::Vertiport as i32,
            end_type: grpc_server::NodeType::Vertiport as i32,
            limit: 0,
            max_range_meters: 1000.0,
        };

        let result = sanitize(request).unwrap_err();
        assert_eq!(result, NNError::InvalidLimit);
    }

    #[test]
    fn ut_sanitize_invalid_range() {
        let request = NearestNeighborRequest {
            start_node_id: uuid::Uuid::new_v4().to_string(),
            start_type: grpc_server::NodeType::Vertiport as i32,
            end_type: grpc_server::NodeType::Vertiport as i32,
            limit: 10,
            max_range_meters: -1.0,
        };

        let result = sanitize(request).unwrap_err();
        assert_eq!(result, NNError::InvalidRange);
    }

    #[test]
    fn ut_sanitize_invalid_path_type() {
        let request = NearestNeighborRequest {
            start_node_id: uuid::Uuid::new_v4().to_string(),
            start_type: grpc_server::NodeType::Aircraft as i32,
            end_type: grpc_server::NodeType::Aircraft as i32,
            limit: 10,
            max_range_meters: 1000.0,
        };

        let result = sanitize(request).unwrap_err();
        assert_eq!(result, NNError::Unsupported);
    }
}
