//! This module contains functions for routing between nodes.
use super::PostgisError;
use crate::grpc::server::grpc_server::{
    BestPathRequest, NodeType, Path as GrpcPath, PathNode as GrpcPathNode, PointZ as GrpcPointZ,
};
use crate::postgis::aircraft::get_aircraft_pointz;
use crate::postgis::vertiport::get_vertiport_centroidz;
use chrono::Duration;
use lib_common::time::*;
use num_traits::FromPrimitive;
use postgis::ewkb::{LineStringT, PointZ};
use std::collections::{BinaryHeap, VecDeque};

/// Look for waypoints within N meters when routing between two points
///  Saves computation time by doing shortest path on a smaller graph
const WAYPOINT_RANGE_METERS: f32 = 1000.0;

/// Elevations to search for valid paths
const FLIGHT_LEVELS: [f32; 9] = [20.0, 40.0, 50.0, 60.0, 70.0, 80.0, 90.0, 100.0, 120.0];

/// Max distance a flight can travel
const MAX_FLIGHT_DISTANCE_METERS: f32 = 50_000.; // 50KM

/// Max paths to return
const MAX_PATH_COUNT_LIMIT: usize = 5;

impl From<PointZ> for GrpcPointZ {
    fn from(field: PointZ) -> Self {
        Self {
            longitude: field.x,
            latitude: field.y,
            altitude_meters: field.z as f32,
        }
    }
}

#[derive(Debug, Clone)]
struct PathNode {
    node_type: i32,
    identifier: String,
    geom: PointZ,
}

impl PartialEq for PathNode {
    fn eq(&self, other: &Self) -> bool {
        self.identifier == other.identifier
    }
}

#[derive(Debug, Clone)]
struct Path {
    path: Vec<PathNode>,
    distance_traversed_meters: f32,
    distance_to_target_meters: f32,
}

impl Path {
    fn heuristic(&self) -> f32 {
        self.distance_traversed_meters + self.distance_to_target_meters
    }
}

// Reverse the ordering so that the BinaryHeap is a min-heap
impl Ord for Path {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        let oh = other.heuristic();
        let sh = self.heuristic();

        if oh < sh {
            std::cmp::Ordering::Less
        } else if oh > sh {
            std::cmp::Ordering::Greater
        } else {
            std::cmp::Ordering::Equal
        }
    }
}

impl PartialOrd for Path {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl PartialEq for Path {
    fn eq(&self, other: &Self) -> bool {
        self.heuristic() == other.heuristic()
    }
}

impl Eq for Path {}

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

    /// Invalid limit
    InvalidLimit,
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
            PathError::InvalidLimit => write!(f, "Invalid number of paths to return."),
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
    limit: usize,
}

impl TryFrom<BestPathRequest> for PathRequest {
    type Error = PostgisError;

    fn try_from(request: BestPathRequest) -> Result<Self, Self::Error> {
        let Ok(limit) = usize::try_from(request.limit) else {
            postgis_error!(
                "(try_from BestPathRequest) invalid limit on number of paths to return: {:?}",
                request.limit
            );
            return Err(PostgisError::BestPath(PathError::InvalidLimit));
        };

        if limit == 0 || limit > MAX_PATH_COUNT_LIMIT {
            postgis_error!(
                "(try_from BestPathRequest) invalid limit on number of paths to return: {:?}",
                limit
            );
            return Err(PostgisError::BestPath(PathError::InvalidLimit));
        }

        let Some(origin_type) = FromPrimitive::from_i32(request.origin_type) else {
            postgis_error!(
                "(try_from BestPathRequest) invalid start node type: {:?}",
                request.origin_type
            );
            return Err(PostgisError::BestPath(PathError::InvalidStartNode));
        };

        let Ok(_) = super::utils::check_string(
            &request.origin_identifier,
            match origin_type {
                NodeType::Vertiport => crate::postgis::vertiport::IDENTIFIER_REGEX,
                NodeType::Aircraft => crate::postgis::aircraft::IDENTIFIER_REGEX,
                _ => {
                    postgis_error!(
                        "(try_from BestPathRequest) invalid start node type: {:?}",
                        origin_type
                    );
                    return Err(PostgisError::BestPath(PathError::InvalidStartNode));
                }
            },
        ) else {
            postgis_error!(
                "(try_from BestPathRequest) invalid start node identifier: {:?}",
                request.origin_identifier
            );

            return Err(PostgisError::BestPath(PathError::InvalidStartNode));
        };

        let Some(target_type) = FromPrimitive::from_i32(request.target_type) else {
            postgis_error!(
                "(try_from BestPathRequest) invalid end node type: {:?}",
                request.target_type
            );
            return Err(PostgisError::BestPath(PathError::InvalidEndNode));
        };

        let Ok(_) = super::utils::check_string(
            &request.target_identifier,
            match target_type {
                NodeType::Vertiport => crate::postgis::vertiport::IDENTIFIER_REGEX,
                _ => {
                    postgis_error!(
                        "(try_from BestPathRequest) invalid end node type: {:?}",
                        target_type
                    );
                    return Err(PostgisError::BestPath(PathError::InvalidEndNode));
                }
            },
        ) else {
            postgis_error!(
                "(try_from BestPathRequest) invalid end node identifier: {:?}",
                request.target_identifier
            );

            return Err(PostgisError::BestPath(PathError::InvalidEndNode));
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
            return Err(PostgisError::BestPath(PathError::InvalidTimeWindow));
        }

        if time_end < Utc::now() {
            return Err(PostgisError::BestPath(PathError::InvalidEndTime));
        }

        Ok(PathRequest {
            origin_identifier: request.origin_identifier,
            target_identifier: request.target_identifier,
            origin_type,
            target_type,
            time_start,
            time_end,
            limit,
        })
    }
}

/// Modified A* algorithm for finding the best path between two points
///  Potentials are sorted by (distance to target + distance traversed)
async fn mod_a_star(
    pool: &deadpool_postgres::Pool,
    origin_node: PathNode,
    target_node: PathNode,
    time_start: DateTime<Utc>,
    time_end: DateTime<Utc>,
    waypoints: Vec<super::waypoint::Waypoint>,
    limit: usize,
) -> Result<Vec<Path>, PostgisError> {
    postgis_info!("(mod_a_star) entry.");

    // Using a binary heap to store potential paths
    //  means potentials are sorted on insert with O(log n)
    //  worst case time complexity
    let mut potentials: BinaryHeap<Path> = BinaryHeap::new();
    let mut completed: BinaryHeap<Path> = BinaryHeap::new();

    // Get all possible waypoints, including at different
    //  flight elevations
    let mut path_points = waypoints
        .into_iter()
        .flat_map(|w| {
            FLIGHT_LEVELS
                .iter()
                .map(|fl| PathNode {
                    node_type: NodeType::Waypoint as i32,
                    identifier: w.identifier.clone(),
                    geom: PointZ {
                        x: w.geom.x,
                        y: w.geom.y,
                        z: *fl as f64,
                        srid: w.geom.srid,
                    },
                })
                .collect::<Vec<_>>()
        })
        .collect::<VecDeque<PathNode>>();

    // Add the destination as a path point
    path_points.push_front(target_node.clone());

    // Add starting node
    let starting_path = Path {
        path: vec![origin_node.clone()],
        distance_to_target_meters: super::utils::distance_meters(
            &origin_node.geom,
            &target_node.geom,
        ),
        distance_traversed_meters: 0.,
    };
    potentials.push(starting_path);

    let client = pool.get().await.map_err(|e| {
        postgis_error!(
            "(mod_a_star) could not get client from psql connection pool: {}",
            e
        );
        PostgisError::BestPath(PathError::Client)
    })?;

    // TODO(R5): Conditional approval zones
    //  For now all zones are considered no-fly zones
    //  So limit query to one result
    let zone_stmt = crate::postgis::zone::get_zone_intersection_stmt(&client).await?;

    // Run until we have 'limit' paths or we run out of potentials
    while completed.len() < limit && !potentials.is_empty() {
        let Some(current) = potentials.pop() else {
            postgis_error!("(mod_a_star) no path found");
            return Err(PostgisError::BestPath(PathError::NoPath));
        };

        for p in path_points.iter() {
            // Don't backtrack
            if current.path.contains(p) {
                continue;
            }

            let Some(last) = current.path.last() else {
                postgis_error!("(mod_a_star) no last point found");
                return Err(PostgisError::BestPath(PathError::NoPath));
            };

            let distance_meters = super::utils::distance_meters(&last.geom, &p.geom);
            let mut tmp = current.clone();
            tmp.distance_traversed_meters += distance_meters;

            // Don't allow flights to exceed max distance
            if tmp.distance_traversed_meters > MAX_FLIGHT_DISTANCE_METERS {
                continue;
            }

            tmp.path.push(p.clone());
            tmp.distance_to_target_meters =
                super::utils::distance_meters(&p.geom, &target_node.geom);

            // If the path has reached the target, shove it into the
            //  potentials list and move on
            if p.identifier != target_node.identifier {
                potentials.push(tmp);
                continue;
            }

            // If the path has reached the target, do final checks
            //  to ensure flight safety

            // Path 3D linestring for zone intersection check
            let linestring = LineStringT {
                points: tmp.path.iter().map(|p| p.geom).collect::<Vec<PointZ>>(),
                srid: Some(4326),
            };

            // Check if any of the zones overlap this path
            let Ok(result) = client
                .query(
                    &zone_stmt,
                    &[
                        &linestring,
                        &time_start,
                        &time_end,
                        &origin_node.identifier,
                        &target_node.identifier,
                    ],
                )
                .await
            else {
                postgis_error!("(mod_a_star) could not query for zone intersection");
                return Err(PostgisError::BestPath(PathError::DBError));
            };

            // If there are any zone intersection results, this path is invalid
            if !result.is_empty() {
                continue;
            }

            // TODO(R4): Check if this conflicts with other flights' segments

            // Valid routes are pushed
            completed.push(tmp)
        }
    }

    let mut completed = completed.into_sorted_vec();
    completed.reverse();

    Ok(completed)
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
) -> Result<Vec<GrpcPath>, PostgisError> {
    postgis_info!("(best_path) request: {:?}", request);
    let request = PathRequest::try_from(request)?;

    let origin_geom = match request.origin_type {
        NodeType::Vertiport => get_vertiport_centroidz(&request.origin_identifier, pool).await?,
        NodeType::Aircraft => get_aircraft_pointz(&request.origin_identifier, pool).await?,
        _ => {
            postgis_error!(
                "(best_path) invalid node types: {:?} -> {:?}",
                request.origin_type,
                request.target_type
            );
            return Err(PostgisError::BestPath(PathError::InvalidStartNode));
        }
    };

    let target_geom = match request.target_type {
        NodeType::Vertiport => get_vertiport_centroidz(&request.target_identifier, pool).await?,
        _ => {
            postgis_error!(
                "(best_path) invalid node types: {:?} -> {:?}",
                request.origin_type,
                request.target_type
            );
            return Err(PostgisError::BestPath(PathError::InvalidEndNode));
        }
    };

    // Get a subset of waypoints within N meters of the line between the origin and target
    //  This saves computation time by doing shortest path on a smaller graph
    let waypoints = crate::postgis::waypoint::get_waypoints_near_geometry(
        &(postgis::ewkb::GeometryT::LineString(LineStringT {
            points: vec![origin_geom, target_geom],
            srid: Some(4326),
        })),
        WAYPOINT_RANGE_METERS,
        pool,
    )
    .await?;

    postgis_info!("(best_path) origin: {:?}", origin_geom);
    postgis_info!("(best_path) target: {:?}", target_geom);
    postgis_info!("(best_path) nearby waypoints: {:?}", waypoints);

    let origin_node = PathNode {
        node_type: request.origin_type as i32,
        identifier: request.origin_identifier,
        geom: origin_geom,
    };

    let target_node = PathNode {
        node_type: request.target_type as i32,
        identifier: request.target_identifier,
        geom: target_geom,
    };

    let result = mod_a_star(
        pool,
        origin_node,
        target_node,
        request.time_start,
        request.time_end,
        waypoints,
        request.limit,
    )
    .await?;

    Ok(result
        .into_iter()
        .map(|path| GrpcPath {
            path: path
                .path
                .iter()
                .enumerate()
                .map(|(index, p)| GrpcPathNode {
                    index: index as i32,
                    node_type: p.node_type,
                    identifier: p.identifier.clone(),
                    geom: Some(p.geom.into()),
                })
                .collect(),
            distance_meters: path.distance_traversed_meters,
        })
        .collect::<Vec<GrpcPath>>())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::grpc::server::grpc_server;

    #[test]
    fn ut_request_valid() {
        let request = BestPathRequest {
            origin_identifier: uuid::Uuid::new_v4().to_string(),
            target_identifier: uuid::Uuid::new_v4().to_string(),
            origin_type: grpc_server::NodeType::Vertiport as i32,
            target_type: grpc_server::NodeType::Vertiport as i32,
            time_start: None,
            time_end: None,
            limit: 1,
        };

        let result = PathRequest::try_from(request);
        assert!(result.is_ok());
    }

    #[test]
    fn ut_request_invalid_aircraft() {
        let request = BestPathRequest {
            origin_identifier: "      ".to_string(),
            target_identifier: uuid::Uuid::new_v4().to_string(),
            origin_type: grpc_server::NodeType::Aircraft as i32,
            target_type: grpc_server::NodeType::Vertiport as i32,
            time_start: None,
            time_end: None,
            limit: 1,
        };

        let result = PathRequest::try_from(request).unwrap_err();
        assert_eq!(result, PostgisError::BestPath(PathError::InvalidStartNode));
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
            limit: 1,
        };

        let result = PathRequest::try_from(request).unwrap_err();
        assert_eq!(result, PostgisError::BestPath(PathError::InvalidStartNode));
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
            limit: 1,
        };

        let result = PathRequest::try_from(request).unwrap_err();
        assert_eq!(result, PostgisError::BestPath(PathError::InvalidTimeWindow));

        // Start time (assumed) is after current time
        let request = BestPathRequest {
            origin_identifier: uuid::Uuid::new_v4().to_string(),
            target_identifier: uuid::Uuid::new_v4().to_string(),
            origin_type: grpc_server::NodeType::Vertiport as i32,
            target_type: grpc_server::NodeType::Vertiport as i32,
            time_start: None,
            time_end: Some(time_end),
            limit: 1,
        };

        let result = PathRequest::try_from(request).unwrap_err();
        assert_eq!(result, PostgisError::BestPath(PathError::InvalidTimeWindow));

        // End time (assumed) is before start time
        let time_start: Timestamp = (Utc::now() + Duration::days(10)).into();

        let request = BestPathRequest {
            origin_identifier: uuid::Uuid::new_v4().to_string(),
            target_identifier: uuid::Uuid::new_v4().to_string(),
            origin_type: grpc_server::NodeType::Vertiport as i32,
            target_type: grpc_server::NodeType::Vertiport as i32,
            time_start: Some(time_start),
            time_end: None,
            limit: 1,
        };

        let result = PathRequest::try_from(request).unwrap_err();
        assert_eq!(result, PostgisError::BestPath(PathError::InvalidTimeWindow));
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
            limit: 1,
        };

        let result = PathRequest::try_from(request).unwrap_err();
        assert_eq!(result, PostgisError::BestPath(PathError::InvalidEndTime));
    }

    #[test]
    fn ut_request_invalid_limit() {
        // End time (assumed) is before start time
        let time_start: Timestamp = Utc::now().into();
        let time_end: Timestamp = (Utc::now() + Duration::days(1)).into();

        // Won't route for a time in the past
        let mut request = BestPathRequest {
            origin_identifier: uuid::Uuid::new_v4().to_string(),
            target_identifier: uuid::Uuid::new_v4().to_string(),
            origin_type: grpc_server::NodeType::Vertiport as i32,
            target_type: grpc_server::NodeType::Vertiport as i32,
            time_start: Some(time_start),
            time_end: Some(time_end),
            limit: -1,
        };

        let result = PathRequest::try_from(request.clone()).unwrap_err();
        assert_eq!(result, PostgisError::BestPath(PathError::InvalidLimit));

        request.limit = 0;
        let result = PathRequest::try_from(request.clone()).unwrap_err();
        assert_eq!(result, PostgisError::BestPath(PathError::InvalidLimit));

        request.limit = (MAX_PATH_COUNT_LIMIT as i32) + 1;
        let result = PathRequest::try_from(request.clone()).unwrap_err();
        assert_eq!(result, PostgisError::BestPath(PathError::InvalidLimit));
    }

    #[test]
    fn ut_path_order() {
        // End time (assumed) is before start time
        let mut paths: BinaryHeap<Path> = BinaryHeap::new();

        let path1 = Path {
            path: vec![],
            distance_traversed_meters: 2.,
            distance_to_target_meters: 0.,
        };

        let path2 = Path {
            path: vec![],
            distance_traversed_meters: 1.,
            distance_to_target_meters: 0.,
        };

        paths.push(path1);
        paths.push(path2);

        assert_eq!(paths.pop().unwrap().distance_traversed_meters, 1.);
        assert_eq!(paths.pop().unwrap().distance_traversed_meters, 2.);
    }
}
