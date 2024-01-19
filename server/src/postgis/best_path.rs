//! This module contains functions for routing between nodes.
use crate::grpc::server::grpc_server::{BestPathRequest, NodeType, PathSegment};
use chrono::Duration;
use lib_common::time::*;
use num_traits::FromPrimitive;
use postgis::ewkb::Geometry;

// TODO(R4): Include altitude, lanes, corridors
// const ALTITUDE_HARDCODE: f32 = 100.0;

/// Look for waypoints within N meters when routing between two points
///  Saves computation time by doing shortest path on a smaller graph
const WAYPOINT_RANGE_METERS: f64 = 1000.0;

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

    /// DBError error
    DBError,
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
            PathError::Client => write!(f, "Could not get backend client."),
            PathError::DBError => write!(f, "Unknown backend error."),
        }
    }
}

#[derive(Debug)]
struct PathRequest {
    origin_identifier: String,
    target_identifier: String,
    origin_type: NodeType,
    target_type: NodeType,
    time_start: DateTime<Utc>,
    time_end: DateTime<Utc>,
}

impl TryFrom<BestPathRequest> for PathRequest {
    type Error = PathError;

    fn try_from(request: BestPathRequest) -> Result<Self, Self::Error> {
        match num::FromPrimitive::from_i32(request.origin_type) {
            Some(NodeType::Vertiport) => {
                uuid::Uuid::parse_str(&request.origin_identifier)
                    .map_err(|_| PathError::InvalidStartNode)?;
            }
            Some(NodeType::Aircraft) => {
                crate::postgis::aircraft::check_identifier(&request.origin_identifier)
                    .map_err(|_| PathError::InvalidStartNode)?;
            }
            _ => {
                postgis_error!(
                    "(try_from BestPathRequest) invalid start node type: {:?}",
                    request.origin_type
                );
                return Err(PathError::InvalidStartNode);
            }
        }

        let origin_identifier = request.origin_identifier;
        let Some(origin_type) = FromPrimitive::from_i32(request.origin_type) else {
            postgis_error!(
                "(try_from BestPathRequest) invalid start node type: {:?}",
                request.origin_type
            );
            return Err(PathError::InvalidStartNode);
        };

        let target_identifier = request.target_identifier;
        let Some(target_type) = FromPrimitive::from_i32(request.target_type) else {
            postgis_error!(
                "(try_from BestPathRequest) invalid end node type: {:?}",
                request.target_type
            );
            return Err(PathError::InvalidEndNode);
        };

        let time_start: DateTime<Utc> = match request.time_start {
            None => Utc::now(),
            Some(time) => time.into(),
        };

        let time_end: DateTime<Utc> = match request.time_end {
            None => Utc::now() + Duration::days(1),
            Some(time) => time.into(),
        };

        if time_end < time_start {
            return Err(PathError::InvalidTimeWindow);
        }

        if time_end < Utc::now() {
            return Err(PathError::InvalidEndTime);
        }

        Ok(PathRequest {
            origin_identifier,
            target_identifier,
            origin_type,
            target_type,
            time_start,
            time_end,
        })
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
    request: BestPathRequest,
    pool: &deadpool_postgres::Pool,
) -> Result<Vec<PathSegment>, PathError> {
    let request = PathRequest::try_from(request)?;

    let (origin_table, target_table) = match (request.origin_type, request.target_type) {
        (NodeType::Vertiport, NodeType::Vertiport) => ("vertiports", "vertiports"),
        (NodeType::Aircraft, NodeType::Vertiport) => ("aircraft", "vertiports"),
        _ => {
            postgis_error!(
                "(best_path) invalid node types: {:?} -> {:?}",
                request.origin_type,
                request.target_type
            );
            return Err(PathError::InvalidStartNode);
        }
    };

    let client = pool.get().await.map_err(|e| {
        postgis_error!(
            "(best_path) could not get client from psql connection pool: {}",
            e
        );
        PathError::Client
    })?;

    let stmt = client
        .prepare_cached("SELECT arrow.centroid(geom) FROM arrow.$1 WHERE identifier = $2;")
        .await
        .map_err(|e| {
            postgis_error!("(best_path) could not prepare cached statement: {}", e);
            PathError::DBError
        })?;

    let Ok(origin_geom) = client
        .query_one(&stmt, &[&origin_table, &request.origin_identifier])
        .await
        .map_err(|e| {
            postgis_error!("(best_path) could not query origin node: {}", e);
            PathError::DBError
        })?
        .try_get::<_, Geometry>(0)
    else {
        postgis_error!("(best_path) could not get origin node geometry");

        return Err(PathError::DBError);
    };

    let Ok(target_geom) = client
        .query_one(&stmt, &[&target_table, &request.target_identifier])
        .await
        .map_err(|e| {
            postgis_error!("(best_path) could not query target node: {}", e);
            PathError::DBError
        })?
        .try_get::<_, Geometry>(0)
    else {
        postgis_error!("(best_path) could not get target node geometry");

        return Err(PathError::DBError);
    };

    // Get a subset of waypoints within N meters of the line between the origin and target
    //  This saves computation time by doing shortest path on a smaller graph
    let stmt = client
        .prepare_cached(
            "SELECT identifer, geom FROM arrow.waypoints
            WHERE
                ST_DWithin(
                    geo,
                    ST_MakeLine($1, $2),
                    $3,
                    false
                );",
        )
        .await
        .map_err(|e| {
            postgis_error!("(best_path) could not prepare cached statement: {}", e);
            PathError::DBError
        })?;

    let waypoints = client
        .query(&stmt, &[&origin_geom, &target_geom, &WAYPOINT_RANGE_METERS])
        .await
        .map_err(|e| {
            postgis_error!("(best_path) could not query waypoints: {}", e);
            PathError::DBError
        })?;

    if waypoints.is_empty() {
        postgis_warn!(
            "(best_path) no waypoints found within {} meters between {:?} ({}) and {:?} {}",
            WAYPOINT_RANGE_METERS,
            request.origin_identifier,
            request.origin_type.to_string(),
            request.target_identifier,
            request.target_type.to_string()
        );

        return Err(PathError::NoPath);
    }

    println!("(best_path) found {} waypoints", waypoints.len());
    println!("(best_path) origin: {:?}", origin_geom);
    println!("(best_path) target: {:?}", target_geom);
    println!("(best_path) waypoints: {:?}", waypoints);

    let results = vec![];
    // let mut results: Vec<PathSegment> = vec![];
    // for r in &rows {
    //     let origin_type: NodeType = r.get(1);
    //     let origin_latitude: f64 = r.get(2);
    //     let origin_longitude: f64 = r.get(3);
    //     let target_type: NodeType = r.get(4);
    //     let target_latitude: f64 = r.get(5);
    //     let target_longitude: f64 = r.get(6);
    //     let distance_meters: f64 = r.get(7);

    //     let origin_type = Into::<NodeType>::into(origin_type) as i32;
    //     let target_type = Into::<NodeType>::into(target_type) as i32;

    //     results.push(PathSegment {
    //         index: r.get(0),
    //         origin_type,
    //         origin_latitude: origin_latitude as f32,
    //         origin_longitude: origin_longitude as f32,
    //         target_type,
    //         target_latitude: target_latitude as f32,
    //         target_longitude: target_longitude as f32,
    //         distance_meters: distance_meters as f32,
    //         altitude_meters: ALTITUDE_HARDCODE, // TODO(R4): Corridors
    //     });
    // }

    Ok(results)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::grpc::server::grpc_server;
    use crate::test_util::get_psql_pool;

    #[test]
    fn ut_request_valid() {
        let request = BestPathRequest {
            origin_identifier: uuid::Uuid::new_v4().to_string(),
            target_identifier: uuid::Uuid::new_v4().to_string(),
            origin_type: grpc_server::NodeType::Vertiport as i32,
            target_type: grpc_server::NodeType::Vertiport as i32,
            time_start: None,
            time_end: None,
        };

        let result = PathRequest::try_from(request);
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn ut_client_failure() {
        crate::get_log_handle().await;
        ut_info!("(ut_client_failure) start");

        let time_start: Timestamp = Utc::now().into();
        let time_end: Timestamp = (Utc::now() + Duration::minutes(10)).into();

        let request = BestPathRequest {
            origin_identifier: uuid::Uuid::new_v4().to_string(),
            target_identifier: uuid::Uuid::new_v4().to_string(),
            origin_type: grpc_server::NodeType::Aircraft as i32,
            target_type: grpc_server::NodeType::Vertiport as i32,
            time_start: Some(time_start),
            time_end: Some(time_end),
        };

        let result = best_path(request, get_psql_pool().await).await.unwrap_err();
        assert_eq!(result, PathError::Client);

        ut_info!("(ut_client_failure) success");
    }

    #[test]
    fn ut_request_invalid_uuids() {
        let request = BestPathRequest {
            origin_identifier: "Invalid".to_string(),
            target_identifier: uuid::Uuid::new_v4().to_string(),
            origin_type: grpc_server::NodeType::Vertiport as i32,
            target_type: grpc_server::NodeType::Vertiport as i32,
            time_start: None,
            time_end: None,
        };

        let result = PathRequest::try_from(request).unwrap_err();
        assert_eq!(result, PathError::InvalidStartNode);

        let request = BestPathRequest {
            origin_identifier: uuid::Uuid::new_v4().to_string(),
            target_identifier: "Invalid".to_string(),
            origin_type: grpc_server::NodeType::Vertiport as i32,
            target_type: grpc_server::NodeType::Vertiport as i32,
            time_start: None,
            time_end: None,
        };

        let result = PathRequest::try_from(request).unwrap_err();
        assert_eq!(result, PathError::InvalidEndNode);
    }

    #[test]
    fn ut_request_invalid_aircraft() {
        let request = BestPathRequest {
            origin_identifier: "Test-123!".to_string(),
            target_identifier: uuid::Uuid::new_v4().to_string(),
            origin_type: grpc_server::NodeType::Aircraft as i32,
            target_type: grpc_server::NodeType::Vertiport as i32,
            time_start: None,
            time_end: None,
        };

        let result = PathRequest::try_from(request).unwrap_err();
        assert_eq!(result, PathError::InvalidStartNode);
    }

    #[test]
    fn ut_request_invalid_origin_node() {
        let request = BestPathRequest {
            origin_identifier: "test-123".to_string(),
            target_identifier: uuid::Uuid::new_v4().to_string(),
            origin_type: grpc_server::NodeType::Waypoint as i32,
            target_type: grpc_server::NodeType::Vertiport as i32,
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
            origin_identifier: uuid::Uuid::new_v4().to_string(),
            target_identifier: uuid::Uuid::new_v4().to_string(),
            origin_type: grpc_server::NodeType::Vertiport as i32,
            target_type: grpc_server::NodeType::Vertiport as i32,
            time_start: Some(time_start),
            time_end: Some(time_end.clone()),
        };

        let result = PathRequest::try_from(request).unwrap_err();
        assert_eq!(result, PathError::InvalidTimeWindow);

        // Start time (assumed) is after current time
        let request = BestPathRequest {
            origin_identifier: uuid::Uuid::new_v4().to_string(),
            target_identifier: uuid::Uuid::new_v4().to_string(),
            origin_type: grpc_server::NodeType::Vertiport as i32,
            target_type: grpc_server::NodeType::Vertiport as i32,
            time_start: None,
            time_end: Some(time_end),
        };

        let result = PathRequest::try_from(request).unwrap_err();
        assert_eq!(result, PathError::InvalidTimeWindow);

        // End time (assumed) is before start time
        let time_start: Timestamp = (Utc::now() + Duration::days(10)).into();

        let request = BestPathRequest {
            origin_identifier: uuid::Uuid::new_v4().to_string(),
            target_identifier: uuid::Uuid::new_v4().to_string(),
            origin_type: grpc_server::NodeType::Vertiport as i32,
            target_type: grpc_server::NodeType::Vertiport as i32,
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
            origin_identifier: uuid::Uuid::new_v4().to_string(),
            target_identifier: uuid::Uuid::new_v4().to_string(),
            origin_type: grpc_server::NodeType::Vertiport as i32,
            target_type: grpc_server::NodeType::Vertiport as i32,
            time_start: Some(time_start),
            time_end: Some(time_end),
        };

        let result = PathRequest::try_from(request).unwrap_err();
        assert_eq!(result, PathError::InvalidEndTime);
    }
}
