//! This module contains functions for routing between nodes.
use crate::grpc::server::grpc_server::{DistanceTo, NearestNeighborRequest, NodeType};

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

    /// DBError error
    DBError,
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
            NNError::Client => write!(f, "Could not get backend client."),
            NNError::DBError => write!(f, "Unknown backend error."),
        }
    }
}

impl NearestNeighborRequest {
    fn validate(&self) -> Result<(), NNError> {
        if self.limit < 1 {
            postgis_error!("(validate) invalid limit: {}", self.limit);
            return Err(NNError::InvalidLimit);
        }

        if self.max_range_meters < 0.0 {
            postgis_error!(
                "(validate) invalid max range meters: {}",
                self.max_range_meters
            );
            return Err(NNError::InvalidRange);
        }

        Ok(())
    }
}

/// Get the nearest neighboring vertiports to a vertiport
async fn nearest_neighbor_vertiport_source(
    stmt: tokio_postgres::Statement,
    client: deadpool_postgres::Client,
    request: NearestNeighborRequest,
) -> Result<Vec<tokio_postgres::Row>, NNError> {
    let Ok(start_node_id) = Uuid::parse_str(&request.start_node_id) else {
        postgis_error!(
            "(nearest_neighbor_vertiport_source) could not parse start node id into UUID: {}",
            request.start_node_id
        );
        return Err(NNError::InvalidStartNode);
    };

    client
        .query(
            &stmt,
            &[
                &start_node_id,
                &request.limit,
                &(request.max_range_meters as f64),
            ],
        )
        .await
        .map_err(|e| {
            postgis_error!(
                "(nearest_neighbor_vertiport_source) could not request routes: {}",
                e
            );
            NNError::DBError
        })
}

/// Get the nearest neighboring vertiports to an aircraft
async fn nearest_neighbor_aircraft_source(
    stmt: tokio_postgres::Statement,
    client: deadpool_postgres::Client,
    request: NearestNeighborRequest,
) -> Result<Vec<tokio_postgres::Row>, NNError> {
    client
        .query(
            &stmt,
            &[
                &request.start_node_id,
                &request.limit,
                &(request.max_range_meters as f64),
            ],
        )
        .await
        .map_err(|e| {
            postgis_error!(
                "(nearest_neighbor_aircraft_source) could not request routes: {}",
                e
            );
            NNError::DBError
        })
}

/// Nearest neighbor query for nodes
#[cfg(not(tarpaulin_include))]
pub async fn nearest_neighbors(
    request: NearestNeighborRequest,
) -> Result<Vec<DistanceTo>, NNError> {
    request.validate()?;

    let start_type = match num::FromPrimitive::from_i32(request.start_type) {
        Some(NodeType::Vertiport) => {
            uuid::Uuid::parse_str(&request.start_node_id).map_err(|_| NNError::InvalidStartNode)?;
            NodeType::Vertiport
        }
        Some(NodeType::Aircraft) => {
            crate::postgis::aircraft::check_identifier(&request.start_node_id)
                .map_err(|_| NNError::InvalidStartNode)?;
            NodeType::Aircraft
        }
        _ => {
            postgis_error!(
                "(nearest_neighbors) invalid start node type: {:?}",
                request.start_type
            );
            return Err(NNError::Unsupported);
        }
    };

    let end_type = match num::FromPrimitive::from_i32(request.end_type) {
        Some(NodeType::Vertiport) => NodeType::Vertiport,
        _ => {
            postgis_error!(
                "(nearest_neighbors) invalid end node type: {:?}",
                request.end_type
            );
            return Err(NNError::Unsupported);
        }
    };

    let Some(pool) = crate::postgis::DEADPOOL_POSTGIS.get() else {
            postgis_error!(
                "(nearest_neighbors) could not get psql pool.",
                e
            );
            
            return Err(NNError::Client)
        };

    let client = pool.get().await.map_err(|e| {
        postgis_error!(
            "(nearest_neighbors) could not get client from psql connection pool: {}",
            e
        );
        NNError::Client
    })?;

    let target_type = request.end_type;
    let rows = match (start_type, end_type) {
        (NodeType::Vertiport, NodeType::Vertiport) => {
            let query = "SELECT * FROM arrow.nearest_vertiports_to_vertiport($1, $2, $3);";
            postgis_debug!("(nearest_neighbors) query [{}]", query);

            let stmt = client.prepare_cached(query).await.map_err(|e| {
                postgis_error!("(nearest_neighbors) could not prepare statement: {}", e);
                NNError::DBError
            })?;

            nearest_neighbor_vertiport_source(stmt, client, request).await?
        }
        (NodeType::Aircraft, NodeType::Vertiport) => {
            let query = "SELECT * FROM arrow.nearest_vertiports_to_aircraft($1, $2, $3);";
            postgis_debug!("(nearest_neighbors) query [{}]", query);

            let stmt = client.prepare_cached(query).await.map_err(|e| {
                postgis_error!("(nearest_neighbors) could not prepare statement: {}", e);
                NNError::DBError
            })?;

            nearest_neighbor_aircraft_source(stmt, client, request).await?
        }
        _ => {
            postgis_error!(
                "(nearest_neighbors) unsupported path types: {:?}",
                (start_type, end_type)
            );
            return Err(NNError::Unsupported);
        }
    };

    let mut results: Vec<DistanceTo> = vec![];
    for r in &rows {
        let Ok(identifier) = r.try_get(0) else {
            postgis_error!(
                "(nearest_neighbors) could not parse identifier into UUID: {}",
                r.get::<usize, String>(0)
            );
            return Err(NNError::DBError);
        };

        let Ok(distance_meters) = r.try_get(1) else {
            postgis_error!(
                "(nearest_neighbors) could not parse distance into f64: {}",
                r.get::<usize, String>(1)
            );
            return Err(NNError::DBError);
        };

        results.push(DistanceTo {
            identifier: identifier.to_string(),
            target_type,
            distance_meters: distance_meters as f32,
        });
    }

    Ok(results)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::grpc::server::grpc_server;

    #[tokio::test]
    async fn ut_client_failure() {
        let request = NearestNeighborRequest {
            start_node_id: uuid::Uuid::new_v4().to_string(),
            start_type: grpc_server::NodeType::Vertiport as i32,
            end_type: grpc_server::NodeType::Vertiport as i32,
            limit: 10,
            max_range_meters: 1000.0,
        };

        let result = nearest_neighbors(request)
            .await
            .unwrap_err();
        assert_eq!(result, NNError::Client);
    }

    #[tokio::test]
    async fn ut_request_invalid_uuids() {
        let request = NearestNeighborRequest {
            start_node_id: "Invalid".to_string(),
            start_type: grpc_server::NodeType::Vertiport as i32,
            end_type: grpc_server::NodeType::Vertiport as i32,
            limit: 10,
            max_range_meters: 1000.0,
        };

        let result = nearest_neighbors(request)
            .await
            .unwrap_err();
        assert_eq!(result, NNError::InvalidStartNode);
    }

    #[tokio::test]
    async fn ut_request_invalid_aircraft() {
        let request = NearestNeighborRequest {
            start_node_id: "Test-123!".to_string(),
            start_type: grpc_server::NodeType::Aircraft as i32,
            end_type: grpc_server::NodeType::Vertiport as i32,
            limit: 10,
            max_range_meters: 1000.0,
        };

        let result = nearest_neighbors(request)
            .await
            .unwrap_err();
        assert_eq!(result, NNError::InvalidStartNode);
    }

    #[tokio::test]
    async fn ut_request_invalid_start_node() {
        let request = NearestNeighborRequest {
            start_node_id: "Aircraft".to_string(),
            start_type: grpc_server::NodeType::Waypoint as i32,
            end_type: grpc_server::NodeType::Vertiport as i32,
            limit: 10,
            max_range_meters: 1000.0,
        };

        let result = nearest_neighbors(request)
            .await
            .unwrap_err();
        assert_eq!(result, NNError::Unsupported);
    }

    #[tokio::test]
    async fn ut_request_invalid_end_node() {
        let request = NearestNeighborRequest {
            start_node_id: uuid::Uuid::new_v4().to_string(),
            start_type: grpc_server::NodeType::Vertiport as i32,
            end_type: grpc_server::NodeType::Waypoint as i32,
            limit: 10,
            max_range_meters: 1000.0,
        };

        let result = nearest_neighbors(request)
            .await
            .unwrap_err();
        assert_eq!(result, NNError::Unsupported);
    }

    #[tokio::test]
    async fn ut_request_invalid_limit() {
        let request = NearestNeighborRequest {
            start_node_id: uuid::Uuid::new_v4().to_string(),
            start_type: grpc_server::NodeType::Vertiport as i32,
            end_type: grpc_server::NodeType::Vertiport as i32,
            limit: 0,
            max_range_meters: 1000.0,
        };

        let result = nearest_neighbors(request)
            .await
            .unwrap_err();
        assert_eq!(result, NNError::InvalidLimit);
    }

    #[tokio::test]
    async fn ut_request_invalid_range() {
        let request = NearestNeighborRequest {
            start_node_id: uuid::Uuid::new_v4().to_string(),
            start_type: grpc_server::NodeType::Vertiport as i32,
            end_type: grpc_server::NodeType::Vertiport as i32,
            limit: 10,
            max_range_meters: -1.0,
        };

        let result = nearest_neighbors(request)
            .await
            .unwrap_err();
        assert_eq!(result, NNError::InvalidRange);
    }

    #[tokio::test]
    async fn ut_request_invalid_path_type() {
        let request = NearestNeighborRequest {
            start_node_id: uuid::Uuid::new_v4().to_string(),
            start_type: grpc_server::NodeType::Aircraft as i32,
            end_type: grpc_server::NodeType::Aircraft as i32,
            limit: 10,
            max_range_meters: 1000.0,
        };

        let result = nearest_neighbors(request)
            .await
            .unwrap_err();
        assert_eq!(result, NNError::Unsupported);
    }
}
