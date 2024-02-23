//! This module contains functions for updating zones in the PostGIS database.
//! Zones have various restrictions and can be permanent or temporary.

use super::PostgisError;
use super::DEFAULT_SRID;
use crate::grpc::server::grpc_server;
use chrono::{DateTime, Utc};
use deadpool_postgres::Object;
use grpc_server::Zone as RequestZone;
use grpc_server::ZoneType;
use num_traits::FromPrimitive;

/// Allowed characters in a identifier
const IDENTIFIER_REGEX: &str = r"^[\-0-9A-Za-z_\.]{1,255}$";

#[derive(Clone, Debug)]
/// Nodes that aircraft can fly between
pub struct Zone {
    /// A unique identifier for the No-Fly Zone (NOTAM id, etc.)
    pub identifier: String,

    /// The type of zone
    pub zone_type: ZoneType,

    /// The geometry string to feed into PSQL
    pub geom: postgis::ewkb::PolygonZ,

    /// The minimum altitude of the zone
    pub altitude_meters_min: f32,

    /// The maximum altitude of the zone
    pub altitude_meters_max: f32,

    /// The start time of the zone, if applicable
    pub time_start: Option<DateTime<Utc>>,

    /// The end time of the zone, if applicable
    pub time_end: Option<DateTime<Utc>>,
}

/// Possible conversion errors from the GRPC type to GIS type
#[derive(Debug, Copy, Clone, PartialEq)]
pub enum ZoneError {
    /// Invalid timestamp format
    Time,

    /// End time earlier than start time
    TimeOrder,

    /// One or more vertices have an invalid location
    Location,

    /// Invalid Identifier
    Identifier,

    /// No zones provided
    NoZones,

    /// Could not get client
    Client,

    /// DBError error
    DBError,

    /// Invalid zone type
    ZoneType,
}

impl std::fmt::Display for ZoneError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            ZoneError::Time => write!(f, "Invalid timestamp provided."),
            ZoneError::TimeOrder => write!(f, "Start time is later than end time."),
            ZoneError::NoZones => write!(f, "No zones were provided."),
            ZoneError::Location => write!(f, "Invalid location provided."),
            ZoneError::Client => write!(f, "Could not get backend client."),
            ZoneError::DBError => write!(f, "Unknown backend error."),
            ZoneError::Identifier => write!(f, "Invalid identifier provided."),
            ZoneError::ZoneType => write!(f, "Invalid zone type provided."),
        }
    }
}

impl TryFrom<RequestZone> for Zone {
    type Error = ZoneError;

    fn try_from(zone: RequestZone) -> Result<Self, Self::Error> {
        if let Err(e) = super::utils::check_string(&zone.identifier, IDENTIFIER_REGEX) {
            postgis_error!(
                "(try_from RequestZone) Invalid zone identifier: {}; {}",
                zone.identifier,
                e
            );
            return Err(ZoneError::Identifier);
        }

        let time_start = zone.time_start.map(Into::<DateTime<Utc>>::into);
        let time_end = zone.time_end.map(Into::<DateTime<Utc>>::into);

        // The start time must be earlier than the end time if both are provided
        if let Some(ts) = time_start {
            if let Some(te) = time_end {
                if te < ts {
                    postgis_error!("(try_from RequestZone) end time is earlier than start time.");
                    return Err(ZoneError::TimeOrder);
                }
            }
        }

        let geom =
            match super::utils::polygon_from_vertices_z(&zone.vertices, zone.altitude_meters_min) {
                Ok(geom) => geom,
                Err(e) => {
                    postgis_error!(
                        "(try_from RequestZone) Error converting zone polygon: {}",
                        e.to_string()
                    );
                    return Err(ZoneError::Location);
                }
            };

        let Some(zone_type) = FromPrimitive::from_i32(zone.zone_type) else {
            postgis_error!(
                "(try_from RequestZone) Invalid zone type: {}",
                zone.zone_type
            );
            return Err(ZoneError::ZoneType);
        };

        Ok(Zone {
            identifier: zone.identifier,
            zone_type,
            geom,
            altitude_meters_min: zone.altitude_meters_min,
            altitude_meters_max: zone.altitude_meters_max,
            time_start,
            time_end,
        })
    }
}

/// Initialize the vertiports table in the PostGIS database
pub async fn psql_init() -> Result<(), PostgisError> {
    // Create Aircraft Table

    let table_name = format!("{}.zones", super::PSQL_SCHEMA);
    let zonetype_str = "zonetype";
    let statements = vec![
        super::psql_enum_declaration::<ZoneType>(zonetype_str),
        format!(
            "CREATE TABLE IF NOT EXISTS {table_name} (
            id SERIAL UNIQUE NOT NULL,
            identifier VARCHAR(255) UNIQUE NOT NULL PRIMARY KEY,
            zone_type {zonetype_str} NOT NULL,
            geom GEOMETRY(POLYHEDRALSURFACEZ, {DEFAULT_SRID}) NOT NULL,
            altitude_meters_min FLOAT(4) NOT NULL,
            altitude_meters_max FLOAT(4) NOT NULL,
            time_start TIMESTAMPTZ,
            time_end TIMESTAMPTZ,
            last_updated TIMESTAMPTZ
        );"
        ),
        format!("CREATE INDEX IF NOT EXISTS zone_geom_idx ON {table_name} USING GIST (geom);"),
    ];

    super::psql_transaction(statements).await
}

/// Updates zones in the PostGIS database.
pub async fn update_zones(zones: Vec<RequestZone>) -> Result<(), ZoneError> {
    postgis_debug!("(update_zones) entry.");
    if zones.is_empty() {
        postgis_error!("(update_zones) no zones provided.");
        return Err(ZoneError::NoZones);
    }

    let zones: Vec<Zone> = zones
        .into_iter()
        .map(Zone::try_from)
        .collect::<Result<Vec<_>, _>>()?;

    let Some(pool) = crate::postgis::DEADPOOL_POSTGIS.get() else {
        postgis_error!("(update_zones) could not get psql pool.");
        return Err(ZoneError::Client);
    };

    let mut client = pool.get().await.map_err(|e| {
        postgis_error!(
            "(update_zones) could not get client from psql connection pool: {}",
            e
        );
        ZoneError::Client
    })?;

    let transaction = client.transaction().await.map_err(|e| {
        postgis_error!("(update_zones) could not create transaction: {}", e);
        ZoneError::DBError
    })?;

    let stmt = transaction
        .prepare_cached(&format!(
            "\
        INSERT INTO {schema}.zones (
            identifier,
            zone_type,
            geom,
            altitude_meters_min,
            altitude_meters_max,
            time_start,
            time_end,
            last_updated
        )
        VALUES (
            $1,
            $2,
            ST_Extrude($3::GEOMETRY(POLYGONZ, {DEFAULT_SRID}), 0, 0, ($5::FLOAT(4) - $4::FLOAT(4))),
            $4,
            $5,
            $6,
            $7,
            NOW()
        )
        ON CONFLICT (identifier) DO UPDATE SET
            geom = ST_Extrude($3::GEOMETRY(POLYGONZ, {DEFAULT_SRID}), 0, 0, ($5::FLOAT(4) - $4::FLOAT(4))),
            altitude_meters_min = $4,
            altitude_meters_max = $5,
            time_start = $6,
            time_end = $7;
        ",
            schema = super::PSQL_SCHEMA
        ))
        .await
        .map_err(|e| {
            postgis_error!("(update_zones) could not prepare cached statement: {}", e);
            ZoneError::DBError
        })?;

    for zone in &zones {
        transaction
            .execute(
                &stmt,
                &[
                    &zone.identifier,
                    &zone.zone_type,
                    &zone.geom,
                    &zone.altitude_meters_min,
                    &zone.altitude_meters_max,
                    &zone.time_start,
                    &zone.time_end,
                ],
            )
            .await
            .map_err(|e| {
                postgis_error!("(update_zones) could not execute transaction: {}", e);
                ZoneError::DBError
            })?;
    }

    match transaction.commit().await {
        Ok(_) => {
            postgis_debug!("(update_zones) success.");
            Ok(())
        }
        Err(e) => {
            postgis_error!("(update_zones) could not commit transaction: {}", e);
            Err(ZoneError::DBError)
        }
    }
}

/// Prepares a statement that checks zone intersections with the provided geometry
pub async fn get_zone_intersection_stmt(
    client: &Object,
) -> Result<tokio_postgres::Statement, PostgisError> {
    let result = client
        .prepare_cached(&format!(
            "
            SELECT (
                identifier,
                geom,
                zone_type,
                altitude_meters_min,
                altitude_meters_max,
                time_start,
                time_end
            )
            FROM {schema}.zones
            WHERE
                (time_start <= $3 OR time_start IS NULL)
                AND (time_end >= $2 OR time_end IS NULL)
                AND identifier NOT IN ($4, $5)
                AND ST_3DIntersects(geom, $1::GEOMETRY(LINESTRINGZ, {DEFAULT_SRID}))
            LIMIT 1;
        ",
            schema = super::PSQL_SCHEMA
        ))
        .await;
    match result {
        Ok(stmt) => Ok(stmt),
        Err(e) => {
            postgis_error!(
                "(get_zone_intersection_stmt) could not prepare cached statement: {}",
                e
            );
            Err(PostgisError::Zone(ZoneError::DBError))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::grpc::server::grpc_server::Coordinates;
    use crate::postgis::utils;

    fn square(latitude: f64, longitude: f64) -> Vec<(f64, f64)> {
        vec![
            (latitude - 0.0001, longitude - 0.0001),
            (latitude + 0.0001, longitude - 0.0001),
            (latitude + 0.0001, longitude + 0.0001),
            (latitude - 0.0001, longitude + 0.0001),
            (latitude - 0.0001, longitude - 0.0001),
        ]
    }

    #[test]
    fn ut_request_valid() {
        let nodes: Vec<(&str, Vec<(f64, f64)>, f32, f32)> = vec![
            ("NFZ_A", square(52.3745905, 4.9160036), 102.0, 200.0),
            ("NFZ_B", square(52.3749819, 4.9156925), 20.5, 120.0),
            ("NFZ_C", square(52.3752144, 4.9153733), 22.0, 100.0),
        ];

        let zones: Vec<RequestZone> = nodes
            .iter()
            .map(
                |(identifier, points, altitude_min, altitude_max)| RequestZone {
                    identifier: identifier.to_string(),
                    vertices: points
                        .iter()
                        .map(|(latitude, longitude)| Coordinates {
                            latitude: *latitude,
                            longitude: *longitude,
                        })
                        .collect(),
                    altitude_meters_min: *altitude_min,
                    altitude_meters_max: *altitude_max,
                    ..Default::default()
                },
            )
            .collect();

        let converted = zones
            .clone()
            .into_iter()
            .map(Zone::try_from)
            .collect::<Result<Vec<_>, _>>()
            .unwrap();

        assert_eq!(zones.len(), converted.len());

        for (i, nfz) in zones.iter().enumerate() {
            assert_eq!(nfz.identifier, converted[i].identifier);
            assert_eq!(
                utils::polygon_from_vertices_z(&nfz.vertices, nfz.altitude_meters_min).unwrap(),
                converted[i].geom
            );
        }
    }

    #[tokio::test]
    async fn ut_client_failure() {
        let nodes: Vec<(&str, Vec<(f64, f64)>)> = vec![("NFZ", square(52.3745905, 4.9160036))];
        let zone: Vec<RequestZone> = nodes
            .iter()
            .map(|(identifier, points)| RequestZone {
                identifier: identifier.to_string(),
                vertices: points
                    .iter()
                    .map(|(latitude, longitude)| Coordinates {
                        latitude: *latitude,
                        longitude: *longitude,
                    })
                    .collect(),
                ..Default::default()
            })
            .collect();

        let result = update_zones(zone).await.unwrap_err();
        assert_eq!(result, ZoneError::Client);
    }

    #[tokio::test]
    async fn ut_zone_request_to_gis_invalid_identifier() {
        for identifier in &[
            "NULL",
            "Nofly_zone;",
            "'Nofly_zone'",
            "Nofly_zone \'",
            &"X".repeat(1000),
        ] {
            let zones: Vec<RequestZone> = vec![RequestZone {
                identifier: identifier.to_string(),
                vertices: square(52.3745905, 4.9160036)
                    .iter()
                    .map(|(latitude, longitude)| Coordinates {
                        latitude: *latitude,
                        longitude: *longitude,
                    })
                    .collect(),
                ..Default::default()
            }];

            let result = update_zones(zones).await.unwrap_err();
            assert_eq!(result, ZoneError::Identifier);
        }
    }

    #[tokio::test]
    async fn ut_zone_request_to_gis_invalid_no_nodes() {
        let zones: Vec<RequestZone> = vec![];
        let result = update_zones(zones).await.unwrap_err();
        assert_eq!(result, ZoneError::NoZones);
    }

    #[tokio::test]
    async fn ut_zone_request_to_gis_invalid_location() {
        let polygons = vec![
            square(-90., 0.),
            square(90., 0.),
            square(0., -180.),
            square(0., 180.),
        ]; // each of these will crate a square outside of the allowable range of lat, lon

        for polygon in polygons {
            let zones: Vec<RequestZone> = vec![RequestZone {
                identifier: "Nofly_zone".to_string(),
                vertices: polygon
                    .iter()
                    .map(|(latitude, longitude)| Coordinates {
                        latitude: *latitude,
                        longitude: *longitude,
                    })
                    .collect(),
                ..Default::default()
            }];

            let result = update_zones(zones).await.unwrap_err();
            assert_eq!(result, ZoneError::Location);
        }

        let polygons = vec![
            vec![
                (52.3745905, 4.9160036),
                (52.3749819, 4.9156925),
                (52.3752144, 4.9153733),
            ], // not enough vertices
            vec![
                (52.3745905, 4.9160036),
                (52.3749819, 4.9156925),
                (52.3752144, 4.9153733),
                (52.3752144, 4.9153733),
            ], // open polygon
        ];

        for polygon in polygons {
            let zones: Vec<RequestZone> = vec![RequestZone {
                identifier: "Nofly_zone".to_string(),
                vertices: polygon
                    .iter()
                    .map(|(latitude, longitude)| Coordinates {
                        latitude: *latitude,
                        longitude: *longitude,
                    })
                    .collect(),
                ..Default::default()
            }];

            let result = update_zones(zones).await.unwrap_err();
            assert_eq!(result, ZoneError::Location);
        }
    }
}
