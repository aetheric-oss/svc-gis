//! This module contains functions for routing between nodes.
use super::PostgisError;
use super::DEFAULT_SRID;
use crate::grpc::server::grpc_server::{
    BestPathRequest, NodeType, Path as GrpcPath, PathNode as GrpcPathNode, PointZ as GrpcPointZ,
};
use crate::postgis::aircraft::get_aircraft_pointz;
use crate::postgis::flight::FlightError;
use crate::postgis::utils::Segment;
use crate::postgis::vertiport::get_vertiport_centroidz;
use lib_common::time::Duration;
use lib_common::time::*;
use num_traits::FromPrimitive;
use postgis::ewkb::{LineStringT, PointZ};
use std::collections::{BinaryHeap, VecDeque};
use std::fmt::{self, Display, Formatter};

/// Look for waypoints within N meters when routing between two points
///  Saves computation time by doing shortest path on a smaller graph
const WAYPOINT_RANGE_METERS: f32 = 10_000.0;

/// Elevations to search for valid paths
const FLIGHT_LEVELS: [f32; 3] = [40.0, 80.0, 120.0];

/// Max distance a flight can travel
const MAX_FLIGHT_DISTANCE_METERS: f32 = 300_000.;

/// Max number of nodes in best path (to circumvent no fly zones)
const MAX_PATH_NODE_COUNT_LIMIT: usize = 5;

/// Max paths to return
const MAX_PATH_COUNT_LIMIT: usize = 5;

/// Default height above vertipad for last waypoint
/// Only used if ingress and egress points are not defined
const VERTIPORT_APPROACH_ALTITUDE_METERS: f64 = 20.0;

/// Best Path Time Limit
///  ~1 seconds per aircraft availability check
///  Prevent runaway calculation with impossible to reach target
const BEST_PATH_TIME_LIMIT_MS: i64 = 1000;

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

    /// Internal error
    Internal,

    /// Zone Intersection
    ZoneIntersection,

    /// Flight Plan Intersection
    FlightPlanIntersection,
}

impl Display for PathError {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
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
            PathError::Internal => write!(f, "Internal error."),
            PathError::ZoneIntersection => write!(f, "Zone intersection error."),
            PathError::FlightPlanIntersection => write!(f, "Flight plan intersection error."),
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
        let limit = usize::try_from(request.limit).map_err(|_| {
            postgis_error!(
                "invalid limit on number of paths to return: {:?}",
                request.limit
            );

            PostgisError::BestPath(PathError::InvalidLimit)
        })?;

        if limit == 0 || limit > MAX_PATH_COUNT_LIMIT {
            postgis_error!("invalid limit on number of paths to return: {:?}", limit);

            return Err(PostgisError::BestPath(PathError::InvalidLimit));
        }

        let origin_type = FromPrimitive::from_i32(request.origin_type).ok_or_else(|| {
            postgis_error!("invalid start node type: {:?}", request.origin_type);

            PostgisError::BestPath(PathError::InvalidStartNode)
        })?;

        let target_type = FromPrimitive::from_i32(request.target_type).ok_or_else(|| {
            postgis_error!("invalid end node type: {:?}", request.target_type);

            PostgisError::BestPath(PathError::InvalidEndNode)
        })?;

        let regex = match origin_type {
            NodeType::Vertiport => crate::postgis::vertiport::IDENTIFIER_REGEX,
            NodeType::Aircraft => crate::postgis::aircraft::IDENTIFIER_REGEX,
            _ => {
                postgis_error!("invalid start node type: {:?}", origin_type);
                return Err(PostgisError::BestPath(PathError::InvalidStartNode));
            }
        };

        super::utils::check_string(&request.origin_identifier, regex).map_err(|_| {
            postgis_error!(
                "invalid start node identifier: {:?}",
                request.origin_identifier
            );

            PostgisError::BestPath(PathError::InvalidStartNode)
        })?;

        let regex = match target_type {
            NodeType::Vertiport => crate::postgis::vertiport::IDENTIFIER_REGEX,
            _ => {
                postgis_error!("invalid end node type: {:?}", target_type);
                return Err(PostgisError::BestPath(PathError::InvalidEndNode));
            }
        };

        super::utils::check_string(&request.target_identifier, regex).map_err(|_| {
            postgis_error!(
                "invalid end node identifier: {:?}",
                request.target_identifier
            );

            PostgisError::BestPath(PathError::InvalidEndNode)
        })?;

        let time_start: DateTime<Utc> = match request.time_start {
            None => Utc::now(),
            Some(time) => time.into(),
        };

        #[cfg(not(tarpaulin_include))]
        // no_coverage: (Rnever) this will never fail
        let delta = Duration::try_days(1).ok_or_else(|| {
            postgis_error!("could not get time delta for 1 day.");
            PostgisError::BestPath(PathError::InvalidTimeWindow)
        })?;

        let time_end: DateTime<Utc> = match request.time_end {
            None => Utc::now() + delta,
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

/// Checks if the path intersects with any no-fly zones or existing flights
#[cfg(not(tarpaulin_include))]
// no_coverage: (R5) need to run with a real database
pub async fn intersection_checks(
    client: &deadpool_postgres::Client,
    points: Vec<PointZ>,
    distance: f32,
    time_start: DateTime<Utc>,
    time_end: DateTime<Utc>,
    origin_identifier: &str,
    target_identifier: &str,
) -> Result<(), PostgisError> {
    // TODO(R5): This is dependent on the aircraft type
    //  Small drones can come closer to one another than large drones
    //  or rideshare vehicles
    const ALLOWABLE_DISTANCE_M: f64 = 10.0;

    let geom = LineStringT {
        points,
        srid: Some(DEFAULT_SRID),
    };

    // Check if any of the zones overlap this path
    let zone_stmt = crate::postgis::zone::get_zone_intersection_stmt(client).await?;
    if let Ok(row) = client
        .query_one(
            &zone_stmt,
            &[
                &geom,
                &time_start,
                &time_end,
                &origin_identifier,
                &target_identifier,
            ],
        )
        .await
    {
        postgis_debug!("flight path intersects with no-fly zone: {:?}", row);
        return Err(PostgisError::BestPath(PathError::ZoneIntersection));
    }
    // Check if this conflicts with other flights' segments
    let flights_stmt = crate::postgis::flight::get_flight_intersection_stmt(client).await?;
    let result = client
        .query(
            &flights_stmt,
            &[&geom, &ALLOWABLE_DISTANCE_M, &time_start, &time_end],
        )
        .await
        .map_err(|e| {
            postgis_error!(
                "could not query for existing flight paths intersection: {}",
                e
            );
            PostgisError::BestPath(PathError::DBError)
        })?;

    if result.is_empty() {
        postgis_debug!("no flight path intersections.");
        return Ok(());
    }

    postgis_debug!(
        "whole flight path intersects with another whole flight path, checking segments.",
    );

    let stmt = client
        .prepare_cached(
            r#"
            SELECT ("distance_to_path" < $3 OR "distance_to_path" IS NULL) as "conflict"
            FROM ST_3DDistance(
                ST_Transform($1, 4978),
                ST_Transform($2, 4978)
            ) as "distance_to_path"
        "#,
        )
        .await
        .map_err(|e| {
            postgis_error!("could not prepare cached statement: {}", e);
            PostgisError::BestPath(PathError::DBError)
        })?;

    let a_segment = Segment {
        geom,
        time_start,
        time_end,
    };

    for row in result {
        postgis_debug!("row: {:?}", row);
        let b_segment = Segment {
            geom: row.try_get("geom").map_err(|e| {
                postgis_debug!("{e}");
                PostgisError::BestPath(PathError::DBError)
            })?,
            time_start: row.try_get("time_start").map_err(|e| {
                postgis_debug!("{e}");
                PostgisError::BestPath(PathError::DBError)
            })?,
            time_end: row.try_get("time_end").map_err(|e| {
                postgis_debug!("{e}");
                PostgisError::BestPath(PathError::DBError)
            })?,
        };

        let b_distance: f64 = row.try_get("distance").map_err(|e| {
            postgis_debug!("{e}");
            PostgisError::BestPath(PathError::DBError)
        })?;

        match crate::postgis::flight::intersection_check(
            client,
            &stmt,
            ALLOWABLE_DISTANCE_M,
            distance.max(b_distance as f32) / 2.0,
            a_segment.clone(),
            b_segment,
        )
        .await
        {
            Err(PostgisError::FlightPath(FlightError::Intersection)) => {
                return Err(PostgisError::BestPath(PathError::FlightPlanIntersection));
            }
            Err(PostgisError::FlightPath(_)) => {
                return Err(PostgisError::BestPath(PathError::DBError));
            }
            _ => (),
        }
    }

    Ok(())
}

/// Modified A* algorithm for finding the best path between two points
///  Potentials are sorted by (distance to target + distance traversed)
#[cfg(not(tarpaulin_include))]
// no_coverage: (R5) need to run with a real database
async fn mod_a_star(
    origin_node: PathNode,
    target_node: PathNode,
    time_start: DateTime<Utc>,
    time_end: DateTime<Utc>,
    waypoints: Vec<super::waypoint::Waypoint>,
    limit: usize,
) -> Result<Vec<Path>, PostgisError> {
    postgis_debug!("entry.");

    // Using a binary heap to store potential paths
    //  means potentials are sorted on insert with O(log n)
    //  worst case time complexity
    let mut potentials: BinaryHeap<Path> = BinaryHeap::new();
    let mut completed: BinaryHeap<Path> = BinaryHeap::new();

    let pool = crate::postgis::DEADPOOL_POSTGIS.get().ok_or_else(|| {
        postgis_error!("could not get psql pool.");
        PostgisError::BestPath(PathError::Client)
    })?;

    let client = pool.get().await.map_err(|e| {
        postgis_error!("could not get client from psql connection pool: {}", e);
        PostgisError::BestPath(PathError::Client)
    })?;

    // egress path
    let start_points = match FromPrimitive::from_i32(origin_node.node_type) {
        Some(NodeType::Vertiport) => {
            let stmt = format!(
                r#"SELECT "egress_0" as path FROM {vertiport_table_name} WHERE "identifier" = $1"#,
                vertiport_table_name = super::vertiport::get_table_name()
            );

            client
                .query_one(&stmt, &[&origin_node.identifier])
                .await
                .map_err(|e| {
                    postgis_error!("could not get egress path: {}", e);
                    PostgisError::BestPath(PathError::DBError)
                })?
                .try_get::<&str, postgis::ewkb::MultiPointZ>("path")
                .map_err(|e| {
                    postgis_error!("could not get egress path: {}", e);
                    PostgisError::BestPath(PathError::DBError)
                })?
                .points
                .into_iter()
                .enumerate()
                .map(|(i, p)| PathNode {
                    node_type: NodeType::Waypoint as i32,
                    identifier: format!("egress_{}", i),
                    geom: p,
                })
                .collect::<Vec<_>>()
        }
        _ => {
            let mut node = origin_node.clone();
            node.geom.z += VERTIPORT_APPROACH_ALTITUDE_METERS;
            vec![node]
        }
    };

    postgis_debug!("start points: {:?}", start_points);

    // ingress path
    let end_points = match FromPrimitive::from_i32(target_node.node_type) {
        Some(NodeType::Vertiport) => {
            let stmt = format!(
                r#"SELECT "ingress_0" as path FROM {vertiport_table_name} WHERE "identifier" = $1"#,
                vertiport_table_name = super::vertiport::get_table_name()
            );

            client
                .query_one(&stmt, &[&target_node.identifier])
                .await
                .map_err(|e| {
                    postgis_error!("could not get ingress path: {}", e);
                    PostgisError::BestPath(PathError::DBError)
                })?
                .try_get::<&str, postgis::ewkb::MultiPointZ>("path")
                .map_err(|e| {
                    postgis_error!("could not get ingress path: {}", e);
                    PostgisError::BestPath(PathError::DBError)
                })?
                .points
                .into_iter()
                .enumerate()
                .map(|(i, p)| PathNode {
                    node_type: NodeType::Waypoint as i32,
                    identifier: format!("ingress_{}", i),
                    geom: p,
                })
                .collect::<Vec<_>>()
        }
        _ => {
            let mut node = target_node.clone();
            node.geom.z += VERTIPORT_APPROACH_ALTITUDE_METERS;
            vec![node]
        }
    };

    postgis_debug!("end points: {:?}", end_points);

    let target_entrance = end_points.first().ok_or_else(|| {
        postgis_error!("no first point to end vertiport found");
        PostgisError::BestPath(PathError::NoPath)
    })?;

    let starting_path = Path {
        path: start_points.clone(), // must include starting path
        distance_to_target_meters: super::utils::distance_meters(
            &start_points
                .last()
                .ok_or_else(|| {
                    postgis_error!("no last point found");
                    PostgisError::BestPath(PathError::NoPath)
                })?
                .geom,
            &target_entrance.geom,
        ),
        distance_traversed_meters: 0.,
    };

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
    path_points.push_front(target_entrance.clone());

    potentials.push(starting_path);

    // TODO(R6): Conditional approval zones
    //  For now all zones are considered no-fly zones
    //  So limit query to one result

    // Run until we have 'limit' paths or we run out of potentials
    let time_limit = Duration::try_milliseconds(BEST_PATH_TIME_LIMIT_MS).ok_or_else(|| {
        postgis_error!("could not get time limit for path calculation.");
        PostgisError::BestPath(PathError::Internal)
    })?;

    let start_time = Utc::now();
    while completed.len() < limit && !potentials.is_empty() {
        if Utc::now() - start_time > time_limit {
            postgis_warn!("max calculation time reached");
            break;
        }

        let current = potentials.pop().ok_or_else(|| {
            postgis_error!("no path found");
            PostgisError::BestPath(PathError::NoPath)
        })?;

        for p in path_points.iter() {
            // Don't backtrack
            if current.path.contains(p) {
                continue;
            }

            let last = current.path.last().ok_or_else(|| {
                postgis_error!("no last point found");
                PostgisError::BestPath(PathError::NoPath)
            })?;

            let mut tmp = current.clone();
            tmp.distance_traversed_meters += super::utils::distance_meters(&last.geom, &p.geom);

            // Don't allow flights to exceed max distance
            if tmp.distance_traversed_meters > MAX_FLIGHT_DISTANCE_METERS {
                continue;
            }

            tmp.distance_to_target_meters =
                super::utils::distance_meters(&p.geom, &target_entrance.geom);

            // If the path has reached the target, shove it into the
            //  potentials list and move on
            if p.identifier != target_entrance.identifier {
                // Limit the max number of nodes to prevent crazy winding paths
                //  waypoints should only be used to get around a local no-fly zone, to
                //  so the total path length should be 2 (origin and target) plus a limited
                //  number of nodes needed to circumvent 1-2 no-fly zones
                if tmp.path.len() < MAX_PATH_NODE_COUNT_LIMIT {
                    tmp.path.push(p.clone());
                    potentials.push(tmp);
                }

                continue;
            }

            // If target entrance reached, add the end points
            tmp.path.extend(end_points.clone());

            // If the path has reached the target, do final checks
            //  to ensure flight safety

            // Path 3D linestring for zone intersection check
            let points = tmp.path.iter().map(|p| p.geom).collect::<Vec<PointZ>>();

            match intersection_checks(
                &client,
                points,
                tmp.distance_traversed_meters,
                time_start,
                time_end,
                &origin_node.identifier,
                &target_node.identifier,
            )
            .await
            {
                Ok(_) => (),
                Err(PostgisError::BestPath(PathError::ZoneIntersection)) => {
                    continue;
                }
                Err(PostgisError::BestPath(PathError::FlightPlanIntersection)) => {
                    continue;
                }
                Err(e) => {
                    postgis_error!("intersection checks failed: {}", e);
                    return Err(e);
                }
            }

            // Valid routes are pushed
            completed.push(tmp);
            if completed.len() >= limit {
                break;
            }
        }
    }

    let mut completed = completed.into_sorted_vec();
    completed.reverse();

    postgis_debug!("completed paths: {:?}", completed);
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
// no_coverage: (Rnever) need running postgresql instance, not unit testable
pub async fn best_path(request: BestPathRequest) -> Result<Vec<GrpcPath>, PostgisError> {
    postgis_info!("request: {:?}", request);
    let request = PathRequest::try_from(request)?;

    let origin_geom = match request.origin_type {
        NodeType::Vertiport => get_vertiport_centroidz(&request.origin_identifier).await?,
        NodeType::Aircraft => get_aircraft_pointz(&request.origin_identifier).await?,
        _ => {
            postgis_error!(
                "invalid node types: {:?} -> {:?}",
                request.origin_type,
                request.target_type
            );
            return Err(PostgisError::BestPath(PathError::InvalidStartNode));
        }
    };

    let target_geom = match request.target_type {
        NodeType::Vertiport => get_vertiport_centroidz(&request.target_identifier).await?,
        _ => {
            postgis_error!(
                "invalid node types: {:?} -> {:?}",
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
            srid: Some(DEFAULT_SRID),
        })),
        WAYPOINT_RANGE_METERS,
    )
    .await?;

    // postgis_info!("origin: {:?}", origin_geom);
    // postgis_info!("target: {:?}", target_geom);
    // postgis_info!("nearby waypoints: {:?}", waypoints);

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
    use lib_common::uuid::Uuid;

    #[test]
    fn ut_request_valid() {
        let request = BestPathRequest {
            origin_identifier: Uuid::new_v4().to_string(),
            target_identifier: Uuid::new_v4().to_string(),
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
            target_identifier: Uuid::new_v4().to_string(),
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
            target_identifier: Uuid::new_v4().to_string(),
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
        let time_end: Timestamp = (Utc::now() - Duration::try_seconds(1).unwrap()).into();

        // Start time is after end time
        let request = BestPathRequest {
            origin_identifier: Uuid::new_v4().to_string(),
            target_identifier: Uuid::new_v4().to_string(),
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
            origin_identifier: Uuid::new_v4().to_string(),
            target_identifier: Uuid::new_v4().to_string(),
            origin_type: grpc_server::NodeType::Vertiport as i32,
            target_type: grpc_server::NodeType::Vertiport as i32,
            time_start: None,
            time_end: Some(time_end),
            limit: 1,
        };

        let result = PathRequest::try_from(request).unwrap_err();
        assert_eq!(result, PostgisError::BestPath(PathError::InvalidTimeWindow));

        // End time (assumed) is before start time
        let time_start: Timestamp = (Utc::now() + Duration::try_days(10).unwrap()).into();

        let request = BestPathRequest {
            origin_identifier: Uuid::new_v4().to_string(),
            target_identifier: Uuid::new_v4().to_string(),
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
        let time_start: Timestamp = (Utc::now() - Duration::try_days(10).unwrap()).into();
        let time_end: Timestamp = (Utc::now() - Duration::try_seconds(1).unwrap()).into();

        // Won't route for a time in the past
        let request = BestPathRequest {
            origin_identifier: Uuid::new_v4().to_string(),
            target_identifier: Uuid::new_v4().to_string(),
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
        let time_end: Timestamp = (Utc::now() + Duration::try_days(1).unwrap()).into();

        // Won't route for a time in the past
        let mut request = BestPathRequest {
            origin_identifier: Uuid::new_v4().to_string(),
            target_identifier: Uuid::new_v4().to_string(),
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

    #[test]
    fn test_path_error_display() {
        assert_eq!(format!("{}", PathError::NoPath), "No path was found.");
        assert_eq!(
            format!("{}", PathError::InvalidStartNode),
            "Invalid start node."
        );
        assert_eq!(
            format!("{}", PathError::InvalidEndNode),
            "Invalid end node."
        );
        assert_eq!(
            format!("{}", PathError::InvalidStartTime),
            "Invalid start time."
        );
        assert_eq!(
            format!("{}", PathError::InvalidEndTime),
            "Invalid end time."
        );
        assert_eq!(
            format!("{}", PathError::InvalidTimeWindow),
            "Invalid time window."
        );
        assert_eq!(
            format!("{}", PathError::Client),
            "Could not get backend client."
        );
        assert_eq!(format!("{}", PathError::DBError), "Unknown backend error.");
        assert_eq!(
            format!("{}", PathError::InvalidLimit),
            "Invalid number of paths to return."
        );
        assert_eq!(format!("{}", PathError::Internal), "Internal error.");
        assert_eq!(
            format!("{}", PathError::ZoneIntersection),
            "Zone intersection error."
        );
        assert_eq!(
            format!("{}", PathError::FlightPlanIntersection),
            "Flight plan intersection error."
        );
    }

    #[test]
    fn test_partial_eq_path_node() {
        let node = PathNode {
            node_type: 0,
            identifier: "test".to_string(),
            geom: PointZ {
                x: 0.,
                y: 0.,
                z: 0.,
                srid: None,
            },
        };

        let other = PathNode {
            identifier: "test2".to_string(),
            ..node.clone()
        };

        assert_ne!(node, other);

        let other = PathNode {
            node_type: 1,
            geom: PointZ {
                x: 1.,
                y: 1.,
                z: 1.,
                srid: None,
            },
            ..node.clone()
        };
        assert_eq!(node, other);
    }

    #[test]
    fn test_from_pointz() {
        let pointz = PointZ {
            x: rand::random(),
            y: rand::random(),
            z: rand::random(),
            srid: None,
        };

        let grpc_pointz: GrpcPointZ = pointz.into();
        assert_eq!(grpc_pointz.longitude, pointz.x);
        assert_eq!(grpc_pointz.latitude, pointz.y);
        assert_eq!(grpc_pointz.altitude_meters, pointz.z as f32);
    }

    #[test]
    fn test_path_eq() {
        let mut path = Path {
            path: vec![],
            distance_traversed_meters: 0.,
            distance_to_target_meters: 0.,
        };

        let heuristic = path.heuristic();
        assert_eq!(
            heuristic,
            path.distance_to_target_meters + path.distance_traversed_meters
        );

        path.distance_traversed_meters = 1.;
        let heuristic = path.heuristic();
        assert_eq!(
            heuristic,
            path.distance_to_target_meters + path.distance_traversed_meters
        );

        path.distance_to_target_meters = 2.;
        let heuristic = path.heuristic();
        assert_eq!(
            heuristic,
            path.distance_to_target_meters + path.distance_traversed_meters
        );

        let mut other = path.clone();
        assert!(path.eq(&other));

        other.distance_traversed_meters = 2.;
        assert!(!path.eq(&other));

        // ordering is reversed for the min heap, comparison is reversed
        assert!(path > other);

        path.distance_traversed_meters = 10.0;
        assert!(path < other);
    }

    #[test]
    fn test_try_from_path_request() {
        let now = Utc::now();
        let request = BestPathRequest {
            origin_identifier: Uuid::new_v4().to_string(),
            target_identifier: Uuid::new_v4().to_string(),
            origin_type: grpc_server::NodeType::Aircraft as i32,
            target_type: grpc_server::NodeType::Vertiport as i32,
            time_start: Some(now.into()),
            time_end: Some((now + Duration::try_hours(1).unwrap()).into()),
            limit: 1,
        };

        // valid request
        let result = PathRequest::try_from(request.clone()).unwrap();
        assert_eq!(result.origin_identifier, request.origin_identifier);
        assert_eq!(result.target_identifier, request.target_identifier);
        assert_eq!(result.origin_type, NodeType::Aircraft);
        assert_eq!(result.target_type, NodeType::Vertiport);
        assert_eq!(result.limit, request.limit as usize);
        assert_eq!(result.time_start, now);
        assert_eq!(result.time_end, now + Duration::try_hours(1).unwrap());

        // invalid start node
        let tmp = BestPathRequest {
            origin_type: 10000,
            ..request.clone()
        };
        let error = PathRequest::try_from(tmp).unwrap_err();
        assert_eq!(error, PostgisError::BestPath(PathError::InvalidStartNode));
        let tmp = BestPathRequest {
            origin_type: NodeType::Waypoint as i32,
            ..request.clone()
        };
        let error = PathRequest::try_from(tmp).unwrap_err();
        assert_eq!(error, PostgisError::BestPath(PathError::InvalidStartNode));

        // invalid end node
        let tmp = BestPathRequest {
            target_type: 10000,
            ..request.clone()
        };
        let error = PathRequest::try_from(tmp).unwrap_err();
        assert_eq!(error, PostgisError::BestPath(PathError::InvalidEndNode));
        let tmp = BestPathRequest {
            target_type: NodeType::Waypoint as i32,
            ..request.clone()
        };
        let error = PathRequest::try_from(tmp).unwrap_err();
        assert_eq!(error, PostgisError::BestPath(PathError::InvalidEndNode));

        // invalid origin identifier
        let tmp = BestPathRequest {
            origin_identifier: "tes  t".to_string(),
            ..request.clone()
        };
        let error = PathRequest::try_from(tmp).unwrap_err();
        assert_eq!(error, PostgisError::BestPath(PathError::InvalidStartNode));

        // invalid target identifier
        let tmp = BestPathRequest {
            target_identifier: "tes  t".to_string(),
            ..request.clone()
        };
        let error = PathRequest::try_from(tmp).unwrap_err();
        assert_eq!(error, PostgisError::BestPath(PathError::InvalidEndNode));

        // invalid time window
        let tmp = BestPathRequest {
            time_start: Some(Timestamp::from(Utc::now() + Duration::try_days(1).unwrap())),
            time_end: Some(Timestamp::from(Utc::now())),
            ..request.clone()
        };
        let error = PathRequest::try_from(tmp).unwrap_err();
        assert_eq!(error, PostgisError::BestPath(PathError::InvalidTimeWindow));

        // invalid end time (before Utc::now())
        let time_end = Utc::now() - Duration::try_days(1).unwrap();
        let time_start = time_end - Duration::try_hours(1).unwrap();
        let tmp = BestPathRequest {
            time_end: Some(time_end.into()),
            time_start: Some(time_start.into()),
            ..request.clone()
        };
        let error = PathRequest::try_from(tmp).unwrap_err();
        assert_eq!(error, PostgisError::BestPath(PathError::InvalidEndTime));
    }
}
