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

impl TryFrom<RequestWaypoint> for Waypoint {
    type Error = WaypointError;

    fn try_from(waypoint: RequestWaypoint) -> Result<Self, Self::Error> {
        if let Err(e) = super::utils::check_string(&waypoint.label, LABEL_REGEX, LABEL_MAX_LENGTH) {
            postgis_error!(
                "(try_from RequestWaypoint) Invalid waypoint label: {}; {}",
                waypoint.label,
                e
            );
            return Err(WaypointError::Label);
        }

        let Some(location) = waypoint.location else {
            postgis_error!("(try_from RequestWaypoint) Waypoint has no location.");
            return Err(WaypointError::Location);
        };

        let geom = match super::utils::point_from_vertex(&location) {
            Ok(geom) => geom,
            Err(e) => {
                postgis_error!(
                    "(try_from RequestWaypoint) Error creating point from vertex: {:?}",
                    e
                );
                return Err(WaypointError::Location);
            }
        };

        Ok(Waypoint {
            label: waypoint.label,
            geom,
        })
    }
}

/// Update waypoints in the PostGIS database
pub async fn update_waypoints(
    waypoints: Vec<RequestWaypoint>,
    pool: &deadpool_postgres::Pool,
) -> Result<(), WaypointError> {
    postgis_debug!("(update_waypoints) entry.");
    if waypoints.is_empty() {
        return Err(WaypointError::NoWaypoints);
    }

    let waypoints: Vec<Waypoint> = waypoints
        .into_iter()
        .map(Waypoint::try_from)
        .collect::<Result<Vec<_>, _>>()?;
    let Ok(mut client) = pool.get().await else {
        postgis_error!("(update_waypoints) error getting client.");
        return Err(WaypointError::Unknown);
    };

    let Ok(transaction) = client.transaction().await else {
        postgis_error!("(update_waypoints) error creating transaction.");
        return Err(WaypointError::Unknown);
    };

    let Ok(stmt) = transaction
        .prepare_cached("SELECT arrow.update_waypoint($1, $2)")
        .await
    else {
        postgis_error!("(update_waypoints) error preparing cached statement.");
        return Err(WaypointError::Unknown);
    };

    for waypoint in &waypoints {
        if let Err(e) = transaction
            .execute(&stmt, &[&waypoint.label, &waypoint.geom])
            .await
        {
            postgis_error!("(update_waypoints) error: {}", e);
            return Err(WaypointError::Unknown);
        }
    }

    match transaction.commit().await {
        Ok(_) => {
            postgis_debug!("(update_waypoints) success.");
        }
        Err(e) => {
            postgis_error!("(update_waypoints) error: {}", e);
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
    use crate::test_util::get_psql_pool;

    #[test]
    fn ut_request_valid() {
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

        let converted = waypoints
            .clone()
            .into_iter()
            .map(Waypoint::try_from)
            .collect::<Result<Vec<_>, _>>()
            .unwrap();

        assert_eq!(waypoints.len(), converted.len());

        for (i, waypoint) in waypoints.iter().enumerate() {
            assert_eq!(waypoint.label, converted[i].label);
            let location = waypoint.location.unwrap();
            assert_eq!(
                utils::point_from_vertex(&location).unwrap(),
                converted[i].geom
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

        let result = update_waypoints(waypoints, get_psql_pool().await)
            .await
            .unwrap_err();
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

            let result = update_waypoints(waypoints, get_psql_pool().await)
                .await
                .unwrap_err();
            assert_eq!(result, WaypointError::Label);
        }
    }

    #[tokio::test]
    async fn ut_waypoints_request_to_gis_invalid_no_nodes() {
        let waypoints: Vec<RequestWaypoint> = vec![];
        let result = update_waypoints(waypoints, get_psql_pool().await)
            .await
            .unwrap_err();
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

            let result = update_waypoints(waypoints, get_psql_pool().await)
                .await
                .unwrap_err();
            assert_eq!(result, WaypointError::Location);
        }
    }
}
