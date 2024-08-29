//! This module contains functions for updating zones in the PostGIS database.
//! Zones have various restrictions and can be permanent or temporary.

use super::{PostgisError, DEFAULT_SRID, PSQL_SCHEMA};
use crate::grpc::server::grpc_server;
use deadpool_postgres::Object;
use grpc_server::Zone as RequestZone;
use grpc_server::ZoneType;
use lib_common::time::{DateTime, Utc};
use num_traits::FromPrimitive;
use std::fmt::{self, Display, Formatter};

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

impl Display for ZoneError {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
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

/// Gets a client connection to the PostGIS database
#[cfg(not(tarpaulin_include))]
// no_coverage: (R5) need postgis backend to test
async fn get_client() -> Result<Object, PostgisError> {
    crate::postgis::DEADPOOL_POSTGIS
        .get()
        .ok_or_else(|| {
            postgis_error!("could not get psql pool.");
            PostgisError::Zone(ZoneError::Client)
        })?
        .get()
        .await
        .map_err(|e| {
            postgis_error!("could not get client from psql connection pool: {}", e);
            PostgisError::Zone(ZoneError::Client)
        })
}

impl TryFrom<RequestZone> for Zone {
    type Error = ZoneError;

    fn try_from(zone: RequestZone) -> Result<Self, Self::Error> {
        super::utils::check_string(&zone.identifier, IDENTIFIER_REGEX).map_err(|e| {
            postgis_error!("Invalid identifier: {}; {}", zone.identifier, e);
            ZoneError::Identifier
        })?;

        // The start time must be earlier than the end time if both are provided

        let time_start = zone.time_start.map(|ts| ts.into());
        let time_end = zone.time_end.map(|te| te.into());

        if let Some(ts) = time_start {
            if let Some(te) = time_end {
                if te < ts {
                    postgis_error!("end time is earlier than start time.");
                    return Err(ZoneError::TimeOrder);
                }
            }
        }

        let geom = super::utils::polygon_from_vertices_z(&zone.vertices, zone.altitude_meters_min)
            .map_err(|e| {
                postgis_error!("Error converting zone polygon: {}", e.to_string());
                ZoneError::Location
            })?;

        let zone_type = FromPrimitive::from_i32(zone.zone_type).ok_or_else(|| {
            postgis_error!("Invalid zone type: {}", zone.zone_type);

            ZoneError::ZoneType
        })?;

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

/// Get the table name for the zones table
/// pub(super) so that it can be used by the vertiports module
pub(super) fn get_table_name() -> &'static str {
    static FULL_NAME: &str = const_format::formatcp!(r#""{PSQL_SCHEMA}"."zones""#,);
    FULL_NAME
}

/// Initialize the vertiports table in the PostGIS database
#[cfg(not(tarpaulin_include))]
// no_coverage: (R5) need postgis backend to test
pub async fn psql_init() -> Result<(), PostgisError> {
    // Create Aircraft Table

    let distance_meters = 20.0;
    let distance_degrees = distance_meters / 111_111.0; // rough estimate
    let zonetype_str = "zonetype";
    let table_name = get_table_name();
    let schema_str = format!(r#""{PSQL_SCHEMA}""#);
    let statements = vec![
        super::psql_enum_declaration::<ZoneType>(zonetype_str),
        format!(
            r#"CREATE TABLE IF NOT EXISTS {table_name} (
            "id" SERIAL UNIQUE NOT NULL,
            "identifier" VARCHAR(255) UNIQUE NOT NULL PRIMARY KEY,
            "zone_type" {zonetype_str} NOT NULL,
            "geom" GEOMETRY(POLYHEDRALSURFACEZ, {DEFAULT_SRID}) NOT NULL,
            "altitude_meters_min" FLOAT(4) NOT NULL,
            "altitude_meters_max" FLOAT(4) NOT NULL,
            "time_start" TIMESTAMPTZ,
            "time_end" TIMESTAMPTZ,
            "last_updated" TIMESTAMPTZ
        );"#
        ),
        format!(
            r#"CREATE INDEX IF NOT EXISTS "zone_geom_idx" ON {table_name} USING GIST ("geom");"#
        ),
        format!(
            r#"CREATE OR REPLACE FUNCTION {schema_str}.create_zone_waypoints()
            RETURNS trigger
            AS $create_zone_waypoints$
            DECLARE
                pt RECORD;
                count INTEGER := 0;
            BEGIN
                DELETE FROM {waypoints_table} WHERE "zone_id" = NEW."id";

                FOR pt in
                    SELECT (ST_DumpPoints(ST_Buffer(ST_PatchN(NEW."geom", 1), {distance_degrees}, 'join=mitre mitre_limit=5.0')))."geom"
                LOOP
                    INSERT INTO {waypoints_table} (
                        "identifier",
                        "geog",
                        "zone_id"
                    ) SELECT
                        NEW."id" || '_waypoint_' || count,
                        pt."geom"::GEOGRAPHY,
                        NEW."id"
                    WHERE NOT EXISTS (
                        SELECT "geog" FROM {waypoints_table} "wp"
                        WHERE
                            "wp"."zone_id" <> NEW."id"
                            AND ("wp"."geog" <-> pt."geom"::GEOGRAPHY) < {distance_meters}
                        LIMIT 1
                    )
                    ON CONFLICT ("identifier") DO UPDATE
                    SET
                        "geog" = EXCLUDED."geog";

                    count := count + 1;
                END LOOP;

                RETURN NEW;
            END;
            $create_zone_waypoints$
            LANGUAGE plpgsql;"#,
            waypoints_table = super::waypoint::get_table_name()
        ),
        format!(
            r#"CREATE OR REPLACE FUNCTION {schema_str}.delete_zone_waypoints()
                RETURNS trigger
                AS $delete_zone_waypoints$
                BEGIN
                    DELETE FROM {waypoints_table}
                        WHERE "zone_id" = OLD."id";
                    RETURN OLD;
                END;
                $delete_zone_waypoints$
                LANGUAGE plpgsql;"#,
            waypoints_table = super::waypoint::get_table_name()
        ),
        format!(
            r#"CREATE OR REPLACE TRIGGER create_zone_waypoints AFTER INSERT OR UPDATE ON {table_name}
                FOR EACH ROW EXECUTE FUNCTION {schema_str}.create_zone_waypoints();"#,
        ),
        format!(
            r#"CREATE OR REPLACE TRIGGER delete_zone_waypoints BEFORE DELETE ON {table_name}
            FOR EACH ROW EXECUTE FUNCTION {schema_str}.delete_zone_waypoints();"#,
        ),
    ];

    super::psql_transaction(statements).await
}

/// Updates zones in the PostGIS database.
#[cfg(not(tarpaulin_include))]
// no_coverage: (R5) need postgis backend to test
pub async fn update_zones(zones: Vec<RequestZone>) -> Result<(), PostgisError> {
    postgis_debug!("entry.");
    if zones.is_empty() {
        postgis_error!("no zones provided.");
        return Err(PostgisError::Zone(ZoneError::NoZones));
    }

    let zones: Vec<Zone> = zones
        .into_iter()
        .map(Zone::try_from)
        .collect::<Result<Vec<_>, _>>()
        .map_err(PostgisError::Zone)?;

    let mut client = get_client().await?;
    let transaction = client.transaction().await.map_err(|e| {
        postgis_error!("could not create transaction: {}", e);
        PostgisError::Zone(ZoneError::DBError)
    })?;

    let zone_create_stmt = transaction
        .prepare_cached(&format!(
            r#"INSERT INTO {table_name} (
            "identifier",
            "zone_type",
            "geom",
            "altitude_meters_min",
            "altitude_meters_max",
            "time_start",
            "time_end",
            "last_updated"
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
        ON CONFLICT ("identifier") DO UPDATE
            SET "geom" = EXCLUDED."geom",
            "altitude_meters_min" = EXCLUDED."altitude_meters_min",
            "altitude_meters_max" = EXCLUDED."altitude_meters_max",
            "time_start" = EXCLUDED."time_start",
            "time_end" = EXCLUDED."time_end";
        "#,
            table_name = get_table_name(),
        ))
        .await
        .map_err(|e| {
            postgis_error!("could not prepare cached statement: {}", e);
            PostgisError::Zone(ZoneError::DBError)
        })?;

    for zone in &zones {
        transaction
            .execute(
                &zone_create_stmt,
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
                postgis_error!("could not execute transaction: {}", e);
                PostgisError::Zone(ZoneError::DBError)
            })?;
    }

    transaction.commit().await.map_err(|e| {
        postgis_error!("could not commit transaction: {}", e);
        PostgisError::Zone(ZoneError::DBError)
    })?;

    postgis_debug!("success.");
    Ok(())
}

/// Prepares a statement that checks zone intersections with the provided geometry
#[cfg(not(tarpaulin_include))]
// no_coverage: (R5) need postgis backend to test
pub async fn get_zone_intersection_stmt(
    client: &Object,
) -> Result<tokio_postgres::Statement, PostgisError> {
    let result = client
        .prepare_cached(&format!(
            r#"
            SELECT
                "identifier",
                "geom",
                "zone_type",
                "altitude_meters_min",
                "altitude_meters_max",
                "time_start",
                "time_end"
            FROM {table_name}
            WHERE
                ST_3DIntersects("geom", $1::GEOMETRY(LINESTRINGZ, {DEFAULT_SRID}))
                AND ("time_start" <= $3 OR "time_start" IS NULL)
                AND ("time_end" >= $2 OR "time_end" IS NULL)
                AND "identifier" NOT IN ($4, $5)
            LIMIT 1;
        "#,
            table_name = get_table_name()
        ))
        .await;

    result.map_err(|e| {
        postgis_error!("could not prepare cached statement: {}", e);
        PostgisError::Zone(ZoneError::DBError)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::grpc::server::grpc_server::Coordinates;
    use crate::postgis::utils;
    use lib_common::time::Duration;

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
        assert_eq!(result, PostgisError::Zone(ZoneError::Client));
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
            assert_eq!(result, PostgisError::Zone(ZoneError::Identifier));
        }
    }

    #[tokio::test]
    async fn ut_zone_request_to_gis_invalid_time_order() {
        let zones: Vec<RequestZone> = vec![RequestZone {
            identifier: "identifier".to_string(),
            time_start: Some(Utc::now().into()),
            time_end: Some((Utc::now() - Duration::days(1)).into()),
            ..Default::default()
        }];

        let result = update_zones(zones).await.unwrap_err();
        assert_eq!(result, PostgisError::Zone(ZoneError::TimeOrder));
    }

    #[tokio::test]
    async fn ut_zone_request_to_gis_invalid_zone_type() {
        let zones: Vec<RequestZone> = vec![RequestZone {
            identifier: "identifier".to_string(),
            zone_type: 10000,
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
        assert_eq!(result, PostgisError::Zone(ZoneError::ZoneType));
    }

    #[tokio::test]
    async fn ut_zone_request_to_gis_invalid_no_nodes() {
        let zones: Vec<RequestZone> = vec![];
        let result = update_zones(zones).await.unwrap_err();
        assert_eq!(result, PostgisError::Zone(ZoneError::NoZones));
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
            assert_eq!(result, PostgisError::Zone(ZoneError::Location));
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
            assert_eq!(result, PostgisError::Zone(ZoneError::Location));
        }
    }

    #[test]
    fn test_zone_error_display() {
        assert_eq!(
            format!("{}", ZoneError::Time),
            "Invalid timestamp provided."
        );
        assert_eq!(
            format!("{}", ZoneError::TimeOrder),
            "Start time is later than end time."
        );
        assert_eq!(format!("{}", ZoneError::NoZones), "No zones were provided.");
        assert_eq!(
            format!("{}", ZoneError::Location),
            "Invalid location provided."
        );
        assert_eq!(
            format!("{}", ZoneError::Client),
            "Could not get backend client."
        );
        assert_eq!(format!("{}", ZoneError::DBError), "Unknown backend error.");
        assert_eq!(
            format!("{}", ZoneError::Identifier),
            "Invalid identifier provided."
        );
        assert_eq!(
            format!("{}", ZoneError::ZoneType),
            "Invalid zone type provided."
        );
    }

    #[test]
    fn test_get_table_name() {
        assert_eq!(get_table_name(), format!("\"{PSQL_SCHEMA}\".\"zones\""));
    }
}
