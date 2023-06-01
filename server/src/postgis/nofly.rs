//! This module contains functions for updating no-fly zones in the PostGIS database.
//! No-Fly Zones are permanent or temporary.

use crate::grpc::server::grpc_server;
use crate::postgis::nofly::NoFlyZone as GisNoFlyZone;
use chrono::{DateTime, Utc};
use grpc_server::NoFlyZone as RequestNoFlyZone;
use lib_common::time::timestamp_to_datetime;

/// A no-fly zone polygon must have at least three vertices (a triangle)
/// A closed polygon has the first and last vertex equal
/// Four vertices needed to indicate a closed triangular region
pub const MIN_NUM_NO_FLY_VERTICES: usize = 4;

#[derive(Debug, Clone)]
/// Nodes that aircraft can fly between
pub struct NoFlyZone {
    /// A unique identifier for the No-Fly Zone (NOTAM id, etc.)
    pub label: String,

    /// The vertices of the no-fly zone, in order
    /// The start vertex should match the end vertex (closed polygon)
    /// The (f32, f32) is (latitude, longitude)
    pub vertices: Vec<(f32, f32)>,

    /// The start time of the no-fly zone, if applicable
    pub time_start: Option<DateTime<Utc>>,

    /// The end time of the no-fly zone, if applicable
    pub time_end: Option<DateTime<Utc>>,

    /// The UUID of the vertiport, if applicable
    pub vertiport_id: Option<uuid::Uuid>,
}

/// Possible conversion errors from the GRPC type to GIS type
#[derive(Debug, Copy, Clone, PartialEq)]
pub enum NoFlyZoneError {
    /// Invalid Vertiport ID
    VertiportId,

    /// Invalid timestamp format
    Timestamp,

    /// End time earlier than start time
    TimeOrder,

    /// Less than [`MIN_NUM_NO_FLY_VERTICES`] vertices
    VertexCount,

    /// The first and last vertices do not match (open polygon)
    OpenPolygon,
}

impl std::fmt::Display for NoFlyZoneError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            NoFlyZoneError::VertiportId => write!(f, "Invalid vertiport UUID provided."),
            NoFlyZoneError::Timestamp => write!(f, "Invalid timestamp provided."),
            NoFlyZoneError::TimeOrder => write!(f, "Start time is later than end time."),
            NoFlyZoneError::VertexCount => {
                write!(f, "Not enough vertices to form a closed polygon.")
            }
            NoFlyZoneError::OpenPolygon => write!(f, "First and last vertices do not match."),
        }
    }
}

/// Convert GRPC request no-fly zone type into GIS no-fly type
pub fn nofly_grpc_to_gis(
    req_zones: Vec<RequestNoFlyZone>,
) -> Result<Vec<GisNoFlyZone>, NoFlyZoneError> {
    let mut zones: Vec<GisNoFlyZone> = vec![];
    for zone in &req_zones {
        let vertiport_id = match &zone.vertiport_id {
            Some(vid) => match uuid::Uuid::parse_str(vid) {
                Ok(id) => Some(id),
                Err(e) => {
                    postgis_error!("(nofly_grpc_to_gis) failed to parse vertiport uuid: {}", e);
                    return Err(NoFlyZoneError::VertiportId);
                }
            },
            _ => None,
        };

        let time_start = match &zone.time_start {
            Some(ts) => match timestamp_to_datetime(ts) {
                Some(dt) => Some(dt),
                _ => {
                    postgis_error!("(nofly_grpc_to_gis) failed to parse timestamp: {:?}", ts);
                    return Err(NoFlyZoneError::Timestamp);
                }
            },
            _ => None,
        };

        let time_end = match &zone.time_end {
            Some(ts) => match timestamp_to_datetime(ts) {
                Some(dt) => Some(dt),
                _ => {
                    postgis_error!("(nofly_grpc_to_gis) failed to parse timestamp: {:?}", ts);
                    return Err(NoFlyZoneError::Timestamp);
                }
            },
            _ => None,
        };

        // The start time must be earlier than the end time if both are provided
        if let Some(ts) = time_start {
            if let Some(te) = time_end {
                if te < ts {
                    postgis_error!("(nofly_grpc_to_gis) end time is earlier than start time.");
                    return Err(NoFlyZoneError::TimeOrder);
                }
            }
        }

        // Gather vertices
        let vertices: Vec<(f32, f32)> = zone
            .vertices
            .iter()
            .map(|v| (v.latitude, v.longitude))
            .collect();

        // Must have at least 3 points to form a triangle
        let size = vertices.len();
        if size < MIN_NUM_NO_FLY_VERTICES {
            postgis_error!(
                "(nofly_grpc_to_gis) request vertex count ({}) was less than floor threshold ({})",
                size,
                MIN_NUM_NO_FLY_VERTICES
            );

            return Err(NoFlyZoneError::VertexCount);
        }

        // The beginning and ending vertex must be equal
        if vertices.first() != vertices.last() {
            postgis_error!(
                "(nofly_grpc_to_gis) request first and last vertex not equal (open polygon)."
            );

            return Err(NoFlyZoneError::OpenPolygon);
        }

        let zone = GisNoFlyZone {
            label: zone.label.clone(),
            vertices,
            time_start,
            time_end,
            vertiport_id,
        };

        zones.push(zone);
    }

    Ok(zones)
}

/// Updates no-fly zones in the PostGIS database.
pub async fn update_nofly(zones: Vec<NoFlyZone>, pool: deadpool_postgres::Pool) -> Result<(), ()> {
    postgis_debug!("(postgis update_nofly) entry.");

    // TODO(R4): prepared statement
    for zone in zones {
        let time_start = match zone.time_start {
            Some(t) => format!("'{:?}'", t),
            None => "NULL".to_string(),
        };

        let time_end = match zone.time_end {
            Some(t) => format!("'{:?}'", t),
            None => "NULL".to_string(),
        };

        let vertiport_id = match zone.vertiport_id {
            Some(vid) => vid.to_string(),
            None => "NULL".to_string(),
        };

        // In SRID 4326, Point(X Y) is (longitude latitude)
        let cmd_str = format!(
            "
        INSERT INTO arrow.nofly (label, geom, time_start, time_end, vertiport_id)
            VALUES ('{}', 'SRID=4326;POLYGON(({}))', {}, {}, {})
            ON CONFLICT(label)
                DO UPDATE
                    SET geom = EXCLUDED.geom,
                        time_start = EXCLUDED.time_start,
                        time_end = EXCLUDED.time_end,
                        vertiport_id = EXCLUDED.vertiport_id;",
            zone.label,
            zone.vertices
                .iter()
                .map(|v: &(f32, f32)| format!("{} {}", v.1, v.0))
                .collect::<Vec<String>>()
                .join(","),
            time_start,
            time_end,
            vertiport_id
        );

        match super::execute_psql_cmd(cmd_str, pool.clone()).await {
            Ok(_) => (),
            Err(e) => {
                println!("(postgis update_nofly) error executing command: {:?}", e);
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
    use chrono::Utc;
    use lib_common::time::datetime_to_timestamp;

    #[test]
    fn ut_nofly_request_to_gis_valid() {
        let mut zones: Vec<RequestNoFlyZone> = vec![];

        let vertices: Vec<Coordinates> = vec![(0.0, 0.1), (0.0, 0.2), (0.0, 0.3), (0.0, 0.1)]
            .iter()
            .map(|(x, y)| Coordinates {
                latitude: *x,
                longitude: *y,
            })
            .collect();

        // Most arguments None
        zones.push(RequestNoFlyZone {
            label: "".into(),
            vertices,
            time_start: None,
            time_end: None,
            vertiport_id: None,
        });
        assert!(nofly_grpc_to_gis(zones.clone()).is_ok());
        zones.clear();

        // Most arguments with a value
        let Some(start) = datetime_to_timestamp(&Utc::now()) else {
            panic!();
        };

        let vertices = vec![(0.0, 0.1), (0.0, 0.2), (0.0, 0.3), (0.0, 0.1)]
            .iter()
            .map(|(x, y)| Coordinates {
                latitude: *x,
                longitude: *y,
            })
            .collect();

        zones.push(RequestNoFlyZone {
            label: "".into(),
            vertices,
            time_start: Some(start.clone()),
            time_end: Some(start),
            vertiport_id: Some(uuid::Uuid::new_v4().to_string()),
        });

        assert!(nofly_grpc_to_gis(zones.clone()).is_ok());
        zones.clear();
    }

    #[test]
    fn ut_nofly_request_to_gis_invalid_time_order() {
        let mut zones: Vec<RequestNoFlyZone> = vec![];

        let start_dt = Utc::now();

        // End time is earlier than start time
        let end_dt = start_dt - chrono::Duration::nanoseconds(1);

        let Some(start) = datetime_to_timestamp(&start_dt) else {
            panic!();
        };

        let Some(end) = datetime_to_timestamp(&end_dt) else {
            panic!();
        };

        let vertices = vec![(0.0, 0.1), (0.0, 0.2), (0.0, 0.3), (0.0, 0.1)]
            .iter()
            .map(|(x, y)| Coordinates {
                latitude: *x,
                longitude: *y,
            })
            .collect();

        zones.push(RequestNoFlyZone {
            label: "".into(),
            vertices,
            time_start: Some(start),
            time_end: Some(end),
            vertiport_id: None,
        });

        let result = nofly_grpc_to_gis(zones.clone()).unwrap_err();
        assert_eq!(result, NoFlyZoneError::TimeOrder);
        zones.clear();
    }

    #[test]
    fn ut_nofly_request_to_gis_invalid_vertices() {
        let mut zones: Vec<RequestNoFlyZone> = vec![];

        // Not enough coordinates
        zones.push(RequestNoFlyZone {
            label: "".into(),
            vertices: vec![Coordinates {
                latitude: 0.0,
                longitude: 0.1,
            }],
            time_start: None,
            time_end: None,
            vertiport_id: None,
        });
        let result = nofly_grpc_to_gis(zones.clone()).unwrap_err();
        assert_eq!(result, NoFlyZoneError::VertexCount);
        zones.clear();

        // First and last coordinate do not match
        let vertices = vec![(0.0, 0.1), (0.0, 0.2), (0.0, 0.3), (0.0, 0.4)]
            .iter()
            .map(|(x, y)| Coordinates {
                latitude: *x,
                longitude: *y,
            })
            .collect();

        zones.push(RequestNoFlyZone {
            label: "".into(),
            vertices,
            time_start: None,
            time_end: None,
            vertiport_id: None,
        });

        let result = nofly_grpc_to_gis(zones.clone()).unwrap_err();
        assert_eq!(result, NoFlyZoneError::OpenPolygon);
    }

    #[test]
    fn ut_nofly_request_to_gis_invalid_vertiport_id() {
        let mut zones: Vec<RequestNoFlyZone> = vec![];

        let vertices: Vec<Coordinates> = vec![(0.0, 0.1), (0.0, 0.2), (0.0, 0.3), (0.0, 0.1)]
            .iter()
            .map(|(x, y)| Coordinates {
                latitude: *x,
                longitude: *y,
            })
            .collect();

        // Not enough coordinates
        zones.push(RequestNoFlyZone {
            label: "".into(),
            vertices,
            time_start: None,
            time_end: None,
            vertiport_id: Some("invalid uuid".to_string()),
        });

        let result = nofly_grpc_to_gis(zones.clone()).unwrap_err();
        assert_eq!(result, NoFlyZoneError::VertiportId);
    }
}
