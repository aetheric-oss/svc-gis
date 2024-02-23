//! Updates waypoints in the PostGIS database.

use crate::grpc::server::grpc_server;
use grpc_server::Waypoint as RequestWaypoint;

use super::PostgisError;

/// Allowed characters in a waypoint identifier
const IDENTIFIER_REGEX: &str = r"^[\-0-9A-Za-z_\.]{1,255}$";

/// Possible conversion errors from the GRPC type to GIS type
#[derive(Debug, Copy, Clone, PartialEq)]
pub enum WaypointError {
    /// No Waypoints
    NoWaypoints,

    /// Invalid Identifier
    Identifier,

    /// No Location
    Location,

    /// Could not get client
    Client,

    /// DBError error
    DBError,
}

impl std::fmt::Display for WaypointError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            WaypointError::NoWaypoints => write!(f, "No waypoints were provided."),
            WaypointError::Identifier => write!(f, "Invalid identifier provided."),
            WaypointError::Location => write!(f, "Invalid location provided."),
            WaypointError::Client => write!(f, "Could not get backend client."),
            WaypointError::DBError => write!(f, "Database error."),
        }
    }
}

/// Waypoint type
#[derive(Debug, Clone)]
pub struct Waypoint {
    /// Waypoint identifier
    pub identifier: String,

    /// Waypoint location (no altitude information)
    pub geom: postgis::ewkb::Point, // No height information
}

impl TryFrom<RequestWaypoint> for Waypoint {
    type Error = WaypointError;

    fn try_from(waypoint: RequestWaypoint) -> Result<Self, Self::Error> {
        if let Err(e) = super::utils::check_string(&waypoint.identifier, IDENTIFIER_REGEX) {
            postgis_error!(
                "(try_from RequestWaypoint) Invalid waypoint identifier: {}; {}",
                waypoint.identifier,
                e
            );
            return Err(WaypointError::Identifier);
        }

        let Some(location) = waypoint.location else {
            postgis_error!("(try_from RequestWaypoint) Waypoint has no location.");
            return Err(WaypointError::Location);
        };

        let geom = match super::utils::point_from_vertex(&location) {
            Ok(geom) => geom,
            Err(e) => {
                postgis_error!(
                    "(try_from RequestWaypoint) Error creating point from vertex: {}",
                    e
                );
                return Err(WaypointError::Location);
            }
        };

        Ok(Waypoint {
            identifier: waypoint.identifier,
            geom,
        })
    }
}

/// Initialize the vertiports table in the PostGIS database
pub async fn psql_init() -> Result<(), PostgisError> {
    // Create Aircraft Table
    let table_name = format!("{}.waypoints", super::PSQL_SCHEMA);
    let statements = vec![
        format!(
            "CREATE TABLE IF NOT EXISTS {table_name} (
            identifier VARCHAR(255) UNIQUE NOT NULL,
            geog GEOGRAPHY NOT NULL
        );"
        ),
        format!("CREATE INDEX IF NOT EXISTS waypoints_geog_idx ON {table_name} USING GIST (geog);"),
    ];

    super::psql_transaction(statements).await
}

/// Update waypoints in the PostGIS database
pub async fn update_waypoints(waypoints: Vec<RequestWaypoint>) -> Result<(), WaypointError> {
    postgis_debug!("(update_waypoints) entry.");
    if waypoints.is_empty() {
        return Err(WaypointError::NoWaypoints);
    }

    let waypoints: Vec<Waypoint> = waypoints
        .into_iter()
        .map(Waypoint::try_from)
        .collect::<Result<Vec<_>, _>>()?;

    let Some(pool) = crate::postgis::DEADPOOL_POSTGIS.get() else {
        postgis_error!("(update_waypoints) could not get psql pool.");

        return Err(WaypointError::Client);
    };

    let mut client = pool.get().await.map_err(|e| {
        postgis_error!(
            "(update_waypoints) could not get client from psql connection pool: {}",
            e
        );
        WaypointError::Client
    })?;

    let transaction = client.transaction().await.map_err(|e| {
        postgis_error!("(update_waypoints) could not create transaction: {}", e);
        WaypointError::DBError
    })?;

    let stmt = transaction
        .prepare_cached(&format!(
            "\
        INSERT INTO {schema}.waypoints (identifier, geog)
        VALUES ($1, $2::geography)
        ON CONFLICT (identifier) DO UPDATE SET geog = $2::geography;",
            schema = super::PSQL_SCHEMA
        ))
        .await
        .map_err(|e| {
            postgis_error!(
                "(update_waypoints) could not prepare cached statement: {}",
                e
            );
            WaypointError::DBError
        })?;

    for waypoint in &waypoints {
        transaction
            .execute(&stmt, &[&waypoint.identifier, &waypoint.geom])
            .await
            .map_err(|e| {
                postgis_error!("(update_waypoints) could not execute transaction: {}", e);
                WaypointError::DBError
            })?;
    }

    match transaction.commit().await {
        Ok(_) => {
            postgis_debug!("(update_waypoints) success.");
            Ok(())
        }
        Err(e) => {
            postgis_error!("(update_waypoints) could not commit transaction: {}", e);
            Err(WaypointError::DBError)
        }
    }
}

/// Get a subset of waypoints within N meters of another geometry
///  Make sure the geometry is in the same SRID as the waypoints
///  (4326)
pub async fn get_waypoints_near_geometry(
    geom: &postgis::ewkb::GeometryZ,
    range_meters: f32,
) -> Result<Vec<Waypoint>, PostgisError> {
    let Some(pool) = crate::postgis::DEADPOOL_POSTGIS.get() else {
        postgis_error!("(get_waypoints_near_geometry) could not get psql pool.");

        return Err(PostgisError::Waypoint(WaypointError::Client));
    };

    let client = pool.get().await.map_err(|e| {
        postgis_error!(
            "(get_waypoints_near_geometry) could not get client from psql connection pool: {}",
            e
        );
        PostgisError::Waypoint(WaypointError::Client)
    })?;

    // Get a subset of waypoints within N meters of the line between the origin and target
    //  This saves computation time by doing shortest path on a smaller graph
    let stmt = format!(
        "SELECT identifier, geog FROM {}.waypoints
        WHERE ST_DWithin(
            geog,
            $1::geography, -- ignores Z-axis
            $2::FLOAT(4),
            false
        );",
        super::PSQL_SCHEMA
    );

    Ok(client
        .query(&stmt, &[&geom, &range_meters])
        .await
        .map_err(|e| {
            postgis_error!(
                "(get_waypoints_near_geometry) could not query waypoints: {}",
                e
            );
            PostgisError::Waypoint(WaypointError::DBError)
        })?
        .into_iter()
        .filter_map(|row| {
            let Ok(identifier) = row.try_get(0) else {
                postgis_error!("(get_waypoints_near_geometry) could not get identifier from row.");
                return None;
            };

            let Ok(geom) = row.try_get(1) else {
                postgis_error!("(get_waypoints_near_geometry) could not get geom from row.");
                return None;
            };

            Some(Waypoint { identifier, geom })
        })
        .collect::<Vec<_>>())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::grpc::server::grpc_server::Coordinates;
    use crate::postgis::utils;

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
            .map(|(identifier, latitude, longitude)| RequestWaypoint {
                identifier: identifier.to_string(),
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
            assert_eq!(waypoint.identifier, converted[i].identifier);
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
            .map(|(identifier, latitude, longitude)| RequestWaypoint {
                identifier: identifier.to_string(),
                location: Some(Coordinates {
                    latitude: *latitude,
                    longitude: *longitude,
                }),
            })
            .collect();

        let result = update_waypoints(waypoints).await.unwrap_err();
        assert_eq!(result, WaypointError::Client);
    }

    #[tokio::test]
    async fn ut_waypoints_request_to_gis_invalid_identifier() {
        for identifier in &[
            "NULL",
            "Waypoint;",
            "'Waypoint'",
            "Waypoint A",
            "Waypoint \'",
            &"X".repeat(1000),
        ] {
            let waypoints: Vec<RequestWaypoint> = vec![RequestWaypoint {
                identifier: identifier.to_string(),
                location: Some(Coordinates {
                    latitude: 0.0,
                    longitude: 0.0,
                }),
            }];

            let result = update_waypoints(waypoints).await.unwrap_err();
            assert_eq!(result, WaypointError::Identifier);
        }
    }

    #[tokio::test]
    async fn ut_waypoints_request_to_gis_invalid_no_nodes() {
        let waypoints: Vec<RequestWaypoint> = vec![];
        let result = update_waypoints(waypoints).await.unwrap_err();
        assert_eq!(result, WaypointError::NoWaypoints);
    }

    #[tokio::test]
    async fn ut_waypoints_request_to_gis_invalid_location() {
        let coords = vec![(-90.1, 0.0), (90.1, 0.0), (0.0, -180.1), (0.0, 180.1)];

        for (i, coord) in coords.iter().enumerate() {
            let waypoints: Vec<RequestWaypoint> = vec![RequestWaypoint {
                identifier: format!("Waypoint-{}", i),
                location: Some(Coordinates {
                    latitude: coord.0,
                    longitude: coord.1,
                }),
            }];

            let result = update_waypoints(waypoints).await.unwrap_err();
            assert_eq!(result, WaypointError::Location);
        }
    }
}
