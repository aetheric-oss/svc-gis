//! This module contains functions for routing between nodes.
use crate::grpc::server::grpc_server::{BestPathRequest, NodeType, PathSegment};
use crate::postgis::PathType;
use chrono::{DateTime, Utc};
use uuid::Uuid;

// TODO(R4): Include altitude, lanes, corridors
const ALTITUDE_HARDCODE: f32 = 1000.0;

/// Possible errors with path requests
#[derive(Debug, Copy, Clone, PartialEq)]
pub enum PathError {
    /// No path was found
    NoPath,

    /// Invalid start node
    InvalidStartNode,

    /// Invalid end node
    InvalidEndNode,

    /// Invalid start time
    InvalidStartTime,

    /// Invalid end time
    InvalidEndTime,

    /// Invalid time window
    InvalidTimeWindow,

    /// Could not get client
    Client,

    /// Unknown error
    Unknown,
}

impl std::fmt::Display for PathError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            PathError::NoPath => write!(f, "No path was found."),
            PathError::InvalidStartNode => write!(f, "Invalid start node."),
            PathError::InvalidEndNode => write!(f, "Invalid end node."),
            PathError::InvalidStartTime => write!(f, "Invalid start time."),
            PathError::InvalidEndTime => write!(f, "Invalid end time."),
            PathError::InvalidTimeWindow => write!(f, "Invalid time window."),
            PathError::Client => write!(f, "Could not get client."),
            PathError::Unknown => write!(f, "Unknown error."),
        }
    }
}

#[derive(Debug)]
struct PathRequest {
    node_start_id: String,
    node_uuid_end: Uuid,
    time_start: DateTime<Utc>,
    time_end: DateTime<Utc>,
}

impl TryFrom<BestPathRequest> for PathRequest {
    type Error = PathError;

    fn try_from(request: BestPathRequest) -> Result<Self, Self::Error> {
        match num::FromPrimitive::from_i32(request.start_type) {
            Some(NodeType::Vertiport) => {
                uuid::Uuid::parse_str(&request.node_start_id)
                    .map_err(|_| PathError::InvalidStartNode)?;
            }
            Some(NodeType::Aircraft) => {
                crate::postgis::aircraft::check_callsign(&request.node_start_id)
                    .map_err(|_| PathError::InvalidStartNode)?;
            }
            _ => {
                postgis_error!(
                    "(try_from BestPathRequest) invalid start node type: {:?}",
                    request.start_type
                );
                return Err(PathError::InvalidStartNode);
            }
        }

        let node_start_id = request.node_start_id;
        let node_uuid_end = match uuid::Uuid::parse_str(&request.node_uuid_end) {
            Ok(uuid) => uuid,
            Err(_) => return Err(PathError::InvalidEndNode),
        };

        let time_start: DateTime<Utc> = match request.time_start {
            None => chrono::Utc::now(),
            Some(time) => time.into(),
        };

        let time_end: DateTime<Utc> = match request.time_end {
            None => chrono::Utc::now() + chrono::Duration::days(1),
            Some(time) => time.into(),
        };

        if time_end < time_start {
            return Err(PathError::InvalidTimeWindow);
        }

        if time_end < Utc::now() {
            return Err(PathError::InvalidEndTime);
        }

        Ok(PathRequest {
            node_start_id,
            node_uuid_end,
            time_start,
            time_end,
        })
    }
}

/// Get the best path from a vertiport to another vertiport
async fn best_path_vertiport_source(
    stmt: tokio_postgres::Statement,
    client: deadpool_postgres::Client,
    request: PathRequest,
) -> Result<Vec<tokio_postgres::Row>, PathError> {
    let Ok(node_start_id) = Uuid::parse_str(&request.node_start_id) else {
        postgis_error!("(best_path_vertiport_source) could not parse start node id into UUID: {}", request.node_start_id);
        return Err(PathError::InvalidStartNode);
    };

    match client
        .query(
            &stmt,
            &[
                &node_start_id,
                &request.node_uuid_end,
                &request.time_start,
                &request.time_end,
            ],
        )
        .await
    {
        Ok(results) => Ok(results),
        Err(e) => {
            println!(
                "(best_path_vertiport_source) could not request routes: {}",
                e
            );
            Err(PathError::Unknown)
        }
    }
}

/// Get the best path from a aircraft to another vertiport
async fn best_path_aircraft_source(
    stmt: tokio_postgres::Statement,
    client: deadpool_postgres::Client,
    request: PathRequest,
) -> Result<Vec<tokio_postgres::Row>, PathError> {
    match client
        .query(
            &stmt,
            &[
                &request.node_start_id,
                &request.node_uuid_end,
                &request.time_start,
                &request.time_end,
            ],
        )
        .await
    {
        Ok(results) => Ok(results),
        Err(e) => {
            println!(
                "(best_path_aircraft_source) could not request routes: {}",
                e
            );

            Err(PathError::Unknown)
        }
    }
}

/// The purpose of this initial search is to verify that a flight between two
///  vertiports is physically possible.
///
/// A flight is physically impossible if the two vertiports cannot be
///  connected by a series of lines such that the aircraft never runs out
///  of charge.
///
/// No-Fly zones can extend flights, isolate aircraft, or disable vertiports entirely.
#[cfg(not(tarpaulin_include))]
pub async fn best_path(
    path_type: PathType,
    request: BestPathRequest,
    pool: deadpool_postgres::Pool,
) -> Result<Vec<PathSegment>, PathError> {
    let request = PathRequest::try_from(request)?;
    let client = match pool.get().await {
        Ok(client) => client,
        Err(e) => {
            println!("(best_path) could not get client from pool.");
            println!("(best_path) error: {:?}", e);
            return Err(PathError::Client);
        }
    };

    let rows = match path_type {
        PathType::PortToPort => {
            let Ok(stmt) = client.prepare_cached(
                "SELECT * FROM arrow.best_path_p2p($1, $2, $3, $4);"
            ).await else {
                postgis_error!("(best_path) could not prepare statement.");
                return Err(PathError::Unknown);
            };

            best_path_vertiport_source(stmt, client, request).await?
        }
        PathType::AircraftToPort => {
            let Ok(stmt) = client.prepare_cached(
                "SELECT * FROM arrow.best_path_a2p($1, $2, $3, $4);"
            ).await else {
                postgis_error!("(best_path) could not prepare statement.");
                return Err(PathError::Unknown);
            };

            best_path_aircraft_source(stmt, client, request).await?
        }
    };

    let mut results: Vec<PathSegment> = vec![];
    for r in &rows {
        let start_type: NodeType = r.get(1);
        let start_latitude: f64 = r.get(2);
        let start_longitude: f64 = r.get(3);
        let end_type: NodeType = r.get(4);
        let end_latitude: f64 = r.get(5);
        let end_longitude: f64 = r.get(6);
        let distance_meters: f64 = r.get(7);

        let start_type = Into::<NodeType>::into(start_type) as i32;
        let end_type = Into::<NodeType>::into(end_type) as i32;

        results.push(PathSegment {
            index: r.get(0),
            start_type,
            start_latitude: start_latitude as f32,
            start_longitude: start_longitude as f32,
            end_type,
            end_latitude: end_latitude as f32,
            end_longitude: end_longitude as f32,
            distance_meters: distance_meters as f32,
            altitude_meters: ALTITUDE_HARDCODE, // TODO(R4): Corridors
        });
    }

    Ok(results)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::grpc::server::grpc_server;
    use chrono::{Duration, Utc};
    use prost_wkt_types::Timestamp;

    #[test]
    fn ut_request_valid() {
        let request = BestPathRequest {
            node_start_id: uuid::Uuid::new_v4().to_string(),
            node_uuid_end: uuid::Uuid::new_v4().to_string(),
            start_type: grpc_server::NodeType::Vertiport as i32,
            time_start: None,
            time_end: None,
        };

        let result = PathRequest::try_from(request);
        assert!(result.is_ok());
    }

    #[test]
    fn ut_request_invalid_uuids() {
        let request = BestPathRequest {
            node_start_id: "Invalid".to_string(),
            node_uuid_end: uuid::Uuid::new_v4().to_string(),
            start_type: grpc_server::NodeType::Vertiport as i32,
            time_start: None,
            time_end: None,
        };

        let result = PathRequest::try_from(request).unwrap_err();
        assert_eq!(result, PathError::InvalidStartNode);

        let request = BestPathRequest {
            node_start_id: uuid::Uuid::new_v4().to_string(),
            node_uuid_end: "Invalid".to_string(),
            start_type: grpc_server::NodeType::Vertiport as i32,
            time_start: None,
            time_end: None,
        };

        let result = PathRequest::try_from(request).unwrap_err();
        assert_eq!(result, PathError::InvalidEndNode);
    }

    #[test]
    fn ut_request_invalid_aircraft() {
        let request = BestPathRequest {
            node_start_id: "Test-123!".to_string(),
            node_uuid_end: uuid::Uuid::new_v4().to_string(),
            start_type: grpc_server::NodeType::Aircraft as i32,
            time_start: None,
            time_end: None,
        };

        let result = PathRequest::try_from(request).unwrap_err();
        assert_eq!(result, PathError::InvalidStartNode);
    }

    #[test]
    fn ut_request_invalid_start_node() {
        let request = BestPathRequest {
            node_start_id: "test-123".to_string(),
            node_uuid_end: uuid::Uuid::new_v4().to_string(),
            start_type: grpc_server::NodeType::Waypoint as i32,
            time_start: None,
            time_end: None,
        };

        let result = PathRequest::try_from(request).unwrap_err();
        assert_eq!(result, PathError::InvalidStartNode);
    }

    #[test]
    fn ut_request_invalid_time_window() {
        let time_start: Timestamp = Utc::now().into();
        let time_end: Timestamp = (Utc::now() - Duration::seconds(1)).into();

        // Start time is after end time
        let request = BestPathRequest {
            node_start_id: uuid::Uuid::new_v4().to_string(),
            node_uuid_end: uuid::Uuid::new_v4().to_string(),
            start_type: grpc_server::NodeType::Vertiport as i32,
            time_start: Some(time_start),
            time_end: Some(time_end.clone()),
        };

        let result = PathRequest::try_from(request).unwrap_err();
        assert_eq!(result, PathError::InvalidTimeWindow);

        // Start time (assumed) is after current time
        let request = BestPathRequest {
            node_start_id: uuid::Uuid::new_v4().to_string(),
            node_uuid_end: uuid::Uuid::new_v4().to_string(),
            start_type: grpc_server::NodeType::Vertiport as i32,
            time_start: None,
            time_end: Some(time_end),
        };

        let result = PathRequest::try_from(request).unwrap_err();
        assert_eq!(result, PathError::InvalidTimeWindow);

        // End time (assumed) is before start time
        let time_start: Timestamp = (Utc::now() + Duration::days(10)).into();

        let request = BestPathRequest {
            node_start_id: uuid::Uuid::new_v4().to_string(),
            node_uuid_end: uuid::Uuid::new_v4().to_string(),
            start_type: grpc_server::NodeType::Vertiport as i32,
            time_start: Some(time_start),
            time_end: None,
        };

        let result = PathRequest::try_from(request).unwrap_err();
        assert_eq!(result, PathError::InvalidTimeWindow);
    }

    #[test]
    fn ut_request_invalid_time_end() {
        // End time (assumed) is before start time
        let time_start: Timestamp = (Utc::now() - Duration::days(10)).into();
        let time_end: Timestamp = (Utc::now() - Duration::seconds(1)).into();

        // Won't route for a time in the past
        let request = BestPathRequest {
            node_start_id: uuid::Uuid::new_v4().to_string(),
            node_uuid_end: uuid::Uuid::new_v4().to_string(),
            start_type: grpc_server::NodeType::Vertiport as i32,
            time_start: Some(time_start),
            time_end: Some(time_end),
        };

        let result = PathRequest::try_from(request).unwrap_err();
        assert_eq!(result, PathError::InvalidEndTime);
    }
}
