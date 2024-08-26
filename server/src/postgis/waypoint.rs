//! Updates waypoints in the PostGIS database.

use super::{PostgisError, PSQL_SCHEMA};
use crate::grpc::server::grpc_server;
use deadpool_postgres::Object;
use grpc_server::Waypoint as RequestWaypoint;
use std::fmt::{self, Display, Formatter};

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

impl Display for WaypointError {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        match self {
            WaypointError::NoWaypoints => write!(f, "No waypoints were provided."),
            WaypointError::Identifier => write!(f, "Invalid identifier provided."),
            WaypointError::Location => write!(f, "Invalid location provided."),
            WaypointError::Client => write!(f, "Could not get backend client."),
            WaypointError::DBError => write!(f, "Database error."),
        }
    }
}

/// Gets the name of this module's table
pub fn get_table_name() -> &'static str {
    static FULL_NAME: &str = const_format::formatcp!(r#""{PSQL_SCHEMA}"."waypoints""#,);
    FULL_NAME
}

/// Get a client from the PostGIS connection pool
#[cfg(not(tarpaulin_include))]
// no_coverage: (R5) need running psql backend, integration test
async fn get_client() -> Result<Object, PostgisError> {
    crate::postgis::DEADPOOL_POSTGIS
        .get()
        .ok_or_else(|| {
            postgis_error!("could not get psql pool.");

            PostgisError::Waypoint(WaypointError::Client)
        })?
        .get()
        .await
        .map_err(|e| {
            postgis_error!("could not get client from psql connection pool: {}", e);
            PostgisError::Waypoint(WaypointError::Client)
        })
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
                "Invalid waypoint identifier: {}; {}",
                waypoint.identifier,
                e
            );
            return Err(WaypointError::Identifier);
        }

        let location = waypoint.location.ok_or_else(|| {
            postgis_error!("Waypoint has no location.");
            WaypointError::Location
        })?;

        let geom = super::utils::point_from_vertex(&location).map_err(|e| {
            postgis_error!("Error creating point from vertex: {}", e);

            WaypointError::Location
        })?;

        Ok(Waypoint {
            identifier: waypoint.identifier,
            geom,
        })
    }
}

/// Initialize the vertiports table in the PostGIS database
#[cfg(not(tarpaulin_include))]
// no_coverage: (R5) need running psql backend, integration test
pub async fn psql_init() -> Result<(), PostgisError> {
    // Create Aircraft Table
    let statements = vec![
        format!(
            r#"CREATE TABLE IF NOT EXISTS {table_name} (
            "identifier" VARCHAR(255) UNIQUE NOT NULL,
            "geog" GEOGRAPHY NOT NULL,
            "zone_id" INTEGER,
            CONSTRAINT "fk_zone"
                FOREIGN KEY ("zone_id")
                REFERENCES {zones_table_name} ("id")
        );"#,
            table_name = get_table_name(),
            zones_table_name = super::zone::get_table_name()
        ),
        format!(
            r#"CREATE INDEX IF NOT EXISTS "waypoints_geog_idx" ON {table_name} USING GIST ("geog");"#,
            table_name = get_table_name()
        ),
    ];

    super::psql_transaction(statements).await
}

/// Update waypoints in the PostGIS database
#[cfg(not(tarpaulin_include))]
// no_coverage: (R5) need running psql backend, integration test
pub async fn update_waypoints(waypoints: Vec<RequestWaypoint>) -> Result<(), PostgisError> {
    postgis_debug!("entry.");
    if waypoints.is_empty() {
        return Err(PostgisError::Waypoint(WaypointError::NoWaypoints));
    }

    let waypoints: Vec<Waypoint> = waypoints
        .into_iter()
        .map(Waypoint::try_from)
        .collect::<Result<Vec<_>, _>>()
        .map_err(PostgisError::Waypoint)?;

    let mut client = get_client().await?;
    let transaction = client.transaction().await.map_err(|e| {
        postgis_error!("could not create transaction: {}", e);
        PostgisError::Waypoint(WaypointError::DBError)
    })?;

    let stmt = transaction
        .prepare_cached(&format!(
            r#"INSERT INTO {table_name} (
            "identifier",
            "geog"
        )
        VALUES ($1, $2::geography)
        ON CONFLICT ("identifier")
        DO UPDATE
            SET "geog" = EXCLUDED."geog";
        "#,
            table_name = get_table_name()
        ))
        .await
        .map_err(|e| {
            postgis_error!("could not prepare cached statement: {}", e);
            PostgisError::Waypoint(WaypointError::DBError)
        })?;

    for waypoint in &waypoints {
        transaction
            .execute(&stmt, &[&waypoint.identifier, &waypoint.geom])
            .await
            .map_err(|e| {
                postgis_error!("could not execute transaction: {}", e);
                PostgisError::Waypoint(WaypointError::DBError)
            })?;
    }

    transaction.commit().await.map_err(|e| {
        postgis_error!("could not commit transaction: {}", e);
        PostgisError::Waypoint(WaypointError::DBError)
    })?;

    postgis_debug!("success.");
    Ok(())
}

/// Get a subset of waypoints within N meters of another geometry
///  Make sure the geometry is in the same SRID as the waypoints
///  (4326)
#[cfg(not(tarpaulin_include))]
// no_coverage: (R5) need running psql backend, integration test
pub async fn get_waypoints_near_geometry(
    geom: &postgis::ewkb::GeometryZ,
    range_meters: f32,
) -> Result<Vec<Waypoint>, PostgisError> {
    let client = get_client().await?;

    // Get a subset of waypoints within N meters of the line between the origin and target
    //  This saves computation time by doing shortest path on a smaller graph
    let stmt = format!(
        r#"SELECT
            "identifier",
            "geog"
        FROM {table_name}
        WHERE ST_DWithin(
            "geog",
            $1::geography, -- ignores Z-axis
            $2::FLOAT(4),
            false
        );"#,
        table_name = get_table_name()
    );

    let result = client
        .query(&stmt, &[&geom, &range_meters])
        .await
        .map_err(|e| {
            postgis_error!("could not query waypoints: {}", e);
            PostgisError::Waypoint(WaypointError::DBError)
        })?
        .into_iter()
        .filter_map(|row| {
            let Ok(identifier) = row.try_get("identifier") else {
                postgis_error!("could not get identifier from row.");
                return None;
            };

            let Ok(geom) = row.try_get("geog") else {
                postgis_error!("could not get geom from row.");
                return None;
            };

            Some(Waypoint { identifier, geom })
        })
        .collect::<Vec<_>>();

    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::grpc::server::grpc_server::Coordinates;
    use crate::postgis::utils;

    #[test]
    fn test_get_table_name() {
        assert_eq!(get_table_name(), r#""arrow"."waypoints""#);
    }

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
        assert_eq!(result, PostgisError::Waypoint(WaypointError::Client));
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
            assert_eq!(result, PostgisError::Waypoint(WaypointError::Identifier));
        }
    }

    #[tokio::test]
    async fn ut_waypoints_request_to_gis_invalid_no_nodes() {
        let waypoints: Vec<RequestWaypoint> = vec![];
        let result = update_waypoints(waypoints).await.unwrap_err();
        assert_eq!(result, PostgisError::Waypoint(WaypointError::NoWaypoints));
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
            assert_eq!(result, PostgisError::Waypoint(WaypointError::Location));
        }
    }

    #[test]
    fn test_waypoint_error_display() {
        let error = WaypointError::NoWaypoints;
        assert_eq!(error.to_string(), "No waypoints were provided.");

        let error = WaypointError::Identifier;
        assert_eq!(error.to_string(), "Invalid identifier provided.");

        let error = WaypointError::Location;
        assert_eq!(error.to_string(), "Invalid location provided.");

        let error = WaypointError::Client;
        assert_eq!(error.to_string(), "Could not get backend client.");

        let error = WaypointError::DBError;
        assert_eq!(error.to_string(), "Database error.");
    }
}
