//! Updates waypoints in the PostGIS database.

use crate::grpc::server::grpc_server;
use grpc_server::Waypoint as RequestWaypoint;

/// Maximum length of a waypoint label
const LABEL_MAX_LENGTH: usize = 20;

/// Allowed characters in a waypoint label
const LABEL_REGEX: &str = r"^[a-zA-Z0-9_-]+$";

/// Possible conversion errors from the GRPC type to GIS type
#[derive(Debug, Copy, Clone, PartialEq)]
pub enum WaypointError {
    /// No Waypoints
    NoWaypoints,

    /// Invalid Label
    Label,

    /// No Location
    Location,

    /// Unknown error
    Unknown,
}

impl std::fmt::Display for WaypointError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            WaypointError::NoWaypoints => write!(f, "No waypoints were provided."),
            WaypointError::Label => write!(f, "Invalid label provided."),
            WaypointError::Location => write!(f, "Invalid location provided."),
            WaypointError::Unknown => write!(f, "Unknown error."),
        }
    }
}

struct Waypoint {
    label: String,
    geom: postgis::ewkb::Point,
}

/// Verify that the request inputs are valid
fn sanitize(waypoints: Vec<RequestWaypoint>) -> Result<Vec<Waypoint>, WaypointError> {
    let mut sanitized_waypoints: Vec<Waypoint> = Vec::new();
    if waypoints.is_empty() {
        return Err(WaypointError::NoWaypoints);
    }

    for waypoint in waypoints {
        if let Err(e) = super::utils::check_string(&waypoint.label, LABEL_REGEX, LABEL_MAX_LENGTH) {
            postgis_error!(
                "(sanitize waypoints) Invalid waypoint label: {}; {}",
                waypoint.label,
                e
            );
            return Err(WaypointError::Label);
        }

        let Some(location) = waypoint.location else {
            postgis_error!(
                "(sanitize waypoints) Waypoint has no location."
            );
            return Err(WaypointError::Location);
        };

        let geom = match super::utils::point_from_vertex(&location) {
            Ok(geom) => geom,
            Err(e) => {
                postgis_error!(
                    "(sanitize waypoints) Error creating point from vertex: {:?}",
                    e
                );
                return Err(WaypointError::Location);
            }
        };

        sanitized_waypoints.push(Waypoint {
            label: waypoint.label,
            geom,
        });
    }

    Ok(sanitized_waypoints)
}

/// Update waypoints in the PostGIS database
pub async fn update_waypoints(
    waypoints: Vec<RequestWaypoint>,
    pool: deadpool_postgres::Pool,
) -> Result<(), WaypointError> {
    postgis_debug!("(postgis update_node) entry.");
    let waypoints = sanitize(waypoints)?;

    let Ok(mut client) = pool.get().await else {
        postgis_error!("(postgis update_waypoints) error getting client.");
        return Err(WaypointError::Unknown);
    };

    let Ok(transaction) = client.transaction().await else {
        postgis_error!("(postgis update_waypoints) error creating transaction.");
        return Err(WaypointError::Unknown);
    };

    let Ok(stmt) = transaction.prepare_cached(
        "SELECT arrow.update_waypoint($1, $2)"
    ).await else {
        postgis_error!("(postgis update_waypoints) error preparing cached statement.");
        return Err(WaypointError::Unknown);
    };

    for waypoint in &waypoints {
        if let Err(e) = transaction
            .execute(&stmt, &[&waypoint.label, &waypoint.geom])
            .await
        {
            postgis_error!("(postgis update_waypoints) error: {}", e);
            return Err(WaypointError::Unknown);
        }
    }

    match transaction.commit().await {
        Ok(_) => {
            postgis_debug!("(postgis update_waypoints) success.");
        }
        Err(e) => {
            postgis_error!("(postgis update_waypoints) error: {}", e);
            return Err(WaypointError::Unknown);
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::grpc::server::grpc_server::Coordinates;
    use crate::postgis::utils;
    use deadpool_postgres::{Config, ManagerConfig, Pool, RecyclingMethod, Runtime};
    use tokio_postgres::NoTls;

    fn get_pool() -> Pool {
        let mut cfg = Config::default();
        cfg.dbname = Some("deadpool".to_string());
        cfg.manager = Some(ManagerConfig {
            recycling_method: RecyclingMethod::Fast,
        });
        cfg.create_pool(Some(Runtime::Tokio1), NoTls).unwrap()
    }

    #[test]
    fn ut_sanitize_valid() {
        let nodes = vec![
            ("ORANGE", 52.3745905, 4.9160036),
            ("STRAWBERRY", 52.3749819, 4.9156925),
            ("BANANA", 52.3752144, 4.9153733),
            ("LEMON", 52.3753012, 4.9156845),
            ("RASPBERRY", 52.3750703, 4.9161538),
        ];

        let waypoints: Vec<RequestWaypoint> = nodes
            .iter()
            .map(|(label, latitude, longitude)| RequestWaypoint {
                label: label.to_string(),
                location: Some(Coordinates {
                    latitude: *latitude,
                    longitude: *longitude,
                }),
            })
            .collect();

        let Ok(sanitized) = sanitize(waypoints.clone()) else {
            panic!();
        };

        assert_eq!(waypoints.len(), sanitized.len());

        for (i, waypoint) in waypoints.iter().enumerate() {
            assert_eq!(waypoint.label, sanitized[i].label);
            let location = waypoint.location.unwrap();
            assert_eq!(
                utils::point_from_vertex(&location).unwrap(),
                sanitized[i].geom
            );
        }
    }

    #[tokio::test]
    async fn ut_client_failure() {
        let nodes = vec![("ORANGE", 52.3745905, 4.9160036)];

        let waypoints: Vec<RequestWaypoint> = nodes
            .iter()
            .map(|(label, latitude, longitude)| RequestWaypoint {
                label: label.to_string(),
                location: Some(Coordinates {
                    latitude: *latitude,
                    longitude: *longitude,
                }),
            })
            .collect();

        let result = update_waypoints(waypoints, get_pool()).await.unwrap_err();
        assert_eq!(result, WaypointError::Unknown);
    }

    #[tokio::test]
    async fn ut_waypoints_request_to_gis_invalid_label() {
        for label in &[
            "NULL",
            "Waypoint;",
            "'Waypoint'",
            "Waypoint A",
            "Waypoint \'",
            &"X".repeat(LABEL_MAX_LENGTH + 1),
        ] {
            let waypoints: Vec<RequestWaypoint> = vec![RequestWaypoint {
                label: label.to_string(),
                location: Some(Coordinates {
                    latitude: 0.0,
                    longitude: 0.0,
                }),
            }];

            let result = update_waypoints(waypoints, get_pool()).await.unwrap_err();
            assert_eq!(result, WaypointError::Label);
        }
    }

    #[tokio::test]
    async fn ut_waypoints_request_to_gis_invalid_no_nodes() {
        let waypoints: Vec<RequestWaypoint> = vec![];
        let result = update_waypoints(waypoints, get_pool()).await.unwrap_err();
        assert_eq!(result, WaypointError::NoWaypoints);
    }

    #[tokio::test]
    async fn ut_waypoints_request_to_gis_invalid_location() {
        let coords = vec![(-90.1, 0.0), (90.1, 0.0), (0.0, -180.1), (0.0, 180.1)];

        for (i, coord) in coords.iter().enumerate() {
            let waypoints: Vec<RequestWaypoint> = vec![RequestWaypoint {
                label: format!("Waypoint-{}", i),
                location: Some(Coordinates {
                    latitude: coord.0,
                    longitude: coord.1,
                }),
            }];

            let result = update_waypoints(waypoints, get_pool()).await.unwrap_err();
            assert_eq!(result, WaypointError::Location);
        }
    }
}
