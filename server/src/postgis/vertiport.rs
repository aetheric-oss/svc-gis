//! Updates vertiports in the PostGIS database.

use super::{PostgisError, DEFAULT_SRID, PSQL_SCHEMA};
use crate::grpc::server::grpc_server;
use deadpool_postgres::Object;
use grpc_server::Vertiport as RequestVertiport;
use grpc_server::ZoneType;
use lib_common::time::{DateTime, Utc};
use postgis::ewkb::PointZ;
use std::fmt::{self, Display, Formatter};

/// Allowed characters in a label
pub const IDENTIFIER_REGEX: &str = r"^[\-0-9A-Za-z_\.]{1,255}$";

/// Vertiport overhead no-fly clearance
const VERTIPORT_CLEARANCE_METERS: f32 = 200.0;

/// Possible conversion errors from the GRPC type to GIS type
#[derive(Debug, Copy, Clone, PartialEq)]
pub enum VertiportError {
    /// Invalid Vertiport ID
    VertiportId,

    /// No Vertiports
    NoVertiports,

    /// Invalid Identifier
    Identifier,

    /// Location of one or more vertices is invalid
    Location,

    /// Could not get client
    Client,

    /// DBError error
    DBError,

    /// Timestamp error
    Timestamp,
}

impl Display for VertiportError {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        match self {
            VertiportError::VertiportId => write!(f, "Invalid vertiport ID provided."),
            VertiportError::NoVertiports => write!(f, "No vertiports were provided."),
            VertiportError::Identifier => write!(f, "Invalid label provided."),
            VertiportError::Location => write!(f, "Invalid vertices provided."),
            VertiportError::Client => write!(f, "Could not get backend client."),
            VertiportError::DBError => write!(f, "Unknown backend error."),
            VertiportError::Timestamp => write!(f, "Invalid timestamp provided."),
        }
    }
}

/// Gets the name of this module's table
fn get_table_name() -> &'static str {
    static FULL_NAME: &str = const_format::formatcp!(r#""{PSQL_SCHEMA}"."vertiports""#,);
    FULL_NAME
}

/// Gets a connected postgis client from the pool
#[cfg(not(tarpaulin_include))]
// no_coverage: (R5) needs a PostGIS backend to test
async fn get_client() -> Result<Object, PostgisError> {
    crate::postgis::DEADPOOL_POSTGIS
        .get()
        .ok_or_else(|| {
            postgis_error!("could not get psql pool.");

            PostgisError::Vertiport(VertiportError::Client)
        })?
        .get()
        .await
        .map_err(|e| {
            postgis_error!("could not get client from psql connection pool: {}", e);
            PostgisError::Vertiport(VertiportError::Client)
        })
}

/// Helper Struct for Validating Requests
struct Vertiport {
    identifier: String,
    label: Option<String>,
    geom: postgis::ewkb::PolygonZ,
    altitude_meters_min: f32,
    altitude_meters_max: f32,
    timestamp: DateTime<Utc>,
}

impl TryFrom<RequestVertiport> for Vertiport {
    type Error = VertiportError;

    fn try_from(vertiport: RequestVertiport) -> Result<Self, Self::Error> {
        super::utils::check_string(&vertiport.identifier, IDENTIFIER_REGEX).map_err(|e| {
            postgis_error!(
                "Vertiport {} has invalid identifier {:?}: {}",
                vertiport.identifier,
                vertiport.identifier,
                e
            );

            VertiportError::Identifier
        })?;

        let geom =
            super::utils::polygon_from_vertices_z(&vertiport.vertices, vertiport.altitude_meters)
                .map_err(|e| {
                postgis_error!("Error converting vertiport polygon: {}", e.to_string());
                VertiportError::Location
            })?;

        let timestamp = vertiport.timestamp_network.clone().ok_or_else(|| {
            postgis_error!(
                "Vertiport {} has invalid timestamp {:?}",
                vertiport.identifier,
                vertiport.timestamp_network
            );

            VertiportError::Timestamp
        })?;

        // TODO(R4): Check altitude

        Ok(Vertiport {
            identifier: vertiport.identifier,
            label: vertiport.label,
            geom,
            altitude_meters_min: vertiport.altitude_meters,
            altitude_meters_max: vertiport.altitude_meters + VERTIPORT_CLEARANCE_METERS,
            timestamp: timestamp.into(),
        })
    }
}

/// Initialize the vertiports table in the PostGIS database
#[cfg(not(tarpaulin_include))]
// no_coverage: (R5) needs a PostGIS backend to test
pub async fn psql_init() -> Result<(), PostgisError> {
    // Create Vertiport Table
    let statements = vec![format!(
        r#"CREATE TABLE IF NOT EXISTS {vertiports_table_name} (
            "identifier" VARCHAR(255) UNIQUE PRIMARY KEY NOT NULL,
            "label" VARCHAR(255) NOT NULL,
            "zone_id" INTEGER NOT NULL,
            "geom" GEOMETRY, -- 3D Polygon
            "altitude_meters" FLOAT(4),
            "last_updated" TIMESTAMPTZ,
            CONSTRAINT "fk_zone"
                FOREIGN KEY ("zone_id")
                REFERENCES {zones_table_name} ("id")
        );"#,
        vertiports_table_name = get_table_name(),
        zones_table_name = super::zone::get_table_name(),
    )];

    super::psql_transaction(statements).await
}

/// Update vertiports in the PostGIS database
#[cfg(not(tarpaulin_include))]
// no_coverage: (R5) needs a PostGIS backend to test
pub async fn update_vertiports(vertiports: Vec<RequestVertiport>) -> Result<(), PostgisError> {
    postgis_debug!("entry.");
    if vertiports.is_empty() {
        return Err(PostgisError::Vertiport(VertiportError::NoVertiports));
    }

    let vertiports: Vec<Vertiport> = vertiports
        .into_iter()
        .map(Vertiport::try_from)
        .collect::<Result<Vec<_>, _>>()
        .map_err(PostgisError::Vertiport)?;

    let mut client = get_client().await?;
    let transaction = client.transaction().await.map_err(|e| {
        postgis_error!("could not create transaction: {}", e);
        PostgisError::Vertiport(VertiportError::DBError)
    })?;

    let stmt = transaction
        .prepare_cached(&format!(
            r#"WITH "tmp" AS (
                INSERT INTO {zones_table_name} (
                    "identifier",
                    "geom",
                    "altitude_meters_min",
                    "altitude_meters_max",
                    "zone_type",
                    "last_updated"
                ) VALUES (
                    $1,
                    ST_EXTRUDE(
                        $2::GEOMETRY(POLYGONZ, {DEFAULT_SRID}),
                        0,
                        0,
                        ($4::FLOAT(4) - $3::FLOAT(4))
                    ),
                    $3,
                    $4,
                    $6,
                    $7
                )
                ON CONFLICT ("identifier") DO UPDATE
                SET
                    "geom" = EXCLUDED."geom",
                    "zone_type" = EXCLUDED."zone_type"
                RETURNING "id"
            ) INSERT INTO {vertiports_table_name} (
                "identifier",
                "zone_id",
                "geom",
                "label",
                "altitude_meters",
                "last_updated"
            ) VALUES (
                $1::VARCHAR,
                (SELECT "id" FROM "tmp"),
                $2::GEOMETRY,
                $5::VARCHAR,
                $3::FLOAT(4),
                $7::TIMESTAMPTZ
            )
            ON CONFLICT ("identifier") DO UPDATE
                SET
                    "label" = coalesce($5, {vertiports_table_name}."label"),
                    "zone_id" = EXCLUDED."zone_id",
                    "geom" = EXCLUDED."geom",
                    "altitude_meters" = EXCLUDED."altitude_meters",
                    "last_updated" = EXCLUDED."last_updated";"#,
            vertiports_table_name = get_table_name(),
            zones_table_name = super::zone::get_table_name(),
        ))
        .await
        .map_err(|e| {
            postgis_error!("could not prepare cached statement: {}", e);
            PostgisError::Vertiport(VertiportError::DBError)
        })?;

    for vertiport in &vertiports {
        transaction
            .execute(
                &stmt,
                &[
                    &vertiport.identifier,
                    &vertiport.geom,
                    &vertiport.altitude_meters_min,
                    &vertiport.altitude_meters_max,
                    &vertiport.label,
                    &ZoneType::Port,
                    &vertiport.timestamp,
                ],
            )
            .await
            .map_err(|e| {
                postgis_error!("could not execute transaction: {}", e);
                PostgisError::Vertiport(VertiportError::DBError)
            })?;
    }

    transaction.commit().await.map_err(|e| {
        postgis_error!("could not commit transaction: {}", e);
        PostgisError::Vertiport(VertiportError::DBError)
    })?;

    postgis_debug!("success.");
    Ok(())
}

/// Gets the central PointZ geometry of a vertiport (for routing) given its identifier.
#[cfg(not(tarpaulin_include))]
// no_coverage: (R5) needs a PostGIS backend to test
pub async fn get_vertiport_centroidz(identifier: &str) -> Result<PointZ, PostgisError> {
    postgis_debug!("entry, vertiport: '{identifier}'.");
    let stmt = format!(
        r#"
        SELECT ST_Force3DZ (
            ST_Centroid("geom"),
            "altitude_meters"
        )
        FROM {table_name}
        WHERE "identifier" = $1;"#,
        table_name = get_table_name()
    );

    get_client()
        .await?
        .query_one(&stmt, &[&identifier])
        .await
        .map_err(|e| {
            postgis_error!("query failed: {}", e);
            PostgisError::Vertiport(VertiportError::DBError)
        })?
        .try_get::<_, PointZ>(0)
        .map_err(|e| {
            postgis_error!(
                "zero or more than one records found for vertiport '{identifier}': {}",
                e
            );
            PostgisError::Vertiport(VertiportError::DBError)
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::grpc::server::grpc_server::Coordinates;
    use crate::postgis::utils;
    use lib_common::uuid::Uuid;

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
        let nodes: Vec<(&str, Vec<(f64, f64)>, f32)> = vec![
            ("VertiportA", square(52.3745905, 4.9160036), 10.0),
            ("VertiportB", square(52.3749819, 4.9156925), 20.0),
            ("VertiportC", square(52.3752144, 4.9153733), 30.0),
        ];

        let vertiports: Vec<RequestVertiport> = nodes
            .iter()
            .map(|(label, points, altitude_meters)| RequestVertiport {
                label: Some(label.to_string()),
                vertices: points
                    .iter()
                    .map(|(latitude, longitude)| Coordinates {
                        latitude: *latitude,
                        longitude: *longitude,
                    })
                    .collect(),
                identifier: Uuid::new_v4().to_string(),
                altitude_meters: *altitude_meters,
                timestamp_network: Some(Utc::now().into()),
            })
            .collect();

        let converted = vertiports
            .clone()
            .into_iter()
            .map(Vertiport::try_from)
            .collect::<Result<Vec<_>, _>>()
            .unwrap();

        assert_eq!(vertiports.len(), converted.len());

        for (i, vertiport) in vertiports.iter().enumerate() {
            assert_eq!(vertiport.label, converted[i].label);
            assert_eq!(
                utils::polygon_from_vertices_z(&vertiport.vertices, vertiport.altitude_meters)
                    .unwrap(),
                converted[i].geom
            );
        }
    }

    #[tokio::test]
    async fn ut_client_failure() {
        let nodes: Vec<(&str, Vec<(f64, f64)>)> =
            vec![("Vertiport", square(52.3745905, 4.9160036))];
        let vertiports: Vec<RequestVertiport> = nodes
            .iter()
            .map(|(label, points)| RequestVertiport {
                label: Some(label.to_string()),
                vertices: points
                    .iter()
                    .map(|(latitude, longitude)| Coordinates {
                        latitude: *latitude,
                        longitude: *longitude,
                    })
                    .collect(),
                identifier: Uuid::new_v4().to_string(),
                altitude_meters: 10.0,
                timestamp_network: Some(Utc::now().into()),
            })
            .collect();

        let result = update_vertiports(vertiports).await.unwrap_err();
        assert_eq!(result, PostgisError::Vertiport(VertiportError::Client));
    }

    #[tokio::test]
    async fn ut_vertiports_request_to_gis_invalid_label() {
        for identifier in &[
            "NULL",
            "Vertiport;",
            "'Vertiport'",
            "Vertiport \'",
            &"X".repeat(1000),
        ] {
            let vertiports: Vec<RequestVertiport> = vec![RequestVertiport {
                label: None,
                vertices: square(52.3745905, 4.9160036)
                    .iter()
                    .map(|(latitude, longitude)| Coordinates {
                        latitude: *latitude,
                        longitude: *longitude,
                    })
                    .collect(),
                identifier: identifier.to_string(),
                altitude_meters: 10.0,
                timestamp_network: Some(Utc::now().into()),
            }];

            let result = update_vertiports(vertiports).await.unwrap_err();
            assert_eq!(result, PostgisError::Vertiport(VertiportError::Identifier));
        }
    }

    #[tokio::test]
    async fn ut_vertiports_request_to_gis_invalid_no_nodes() {
        let vertiports: Vec<RequestVertiport> = vec![];
        let result = update_vertiports(vertiports).await.unwrap_err();
        assert_eq!(
            result,
            PostgisError::Vertiport(VertiportError::NoVertiports)
        );
    }

    #[tokio::test]
    async fn ut_vertiports_request_to_gis_invalid_location() {
        let polygons = vec![
            square(-90., 0.),
            square(90., 0.),
            square(0., -180.),
            square(0., 180.),
        ]; // each of these will crate a square outside of the allowable range of lat, lon

        for polygon in polygons {
            let vertiports: Vec<RequestVertiport> = vec![RequestVertiport {
                vertices: polygon
                    .iter()
                    .map(|(latitude, longitude)| Coordinates {
                        latitude: *latitude,
                        longitude: *longitude,
                    })
                    .collect(),
                identifier: Uuid::new_v4().to_string(),
                ..Default::default()
            }];

            let result = update_vertiports(vertiports).await.unwrap_err();
            assert_eq!(result, PostgisError::Vertiport(VertiportError::Location));
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
            let vertiports: Vec<RequestVertiport> = vec![RequestVertiport {
                vertices: polygon
                    .iter()
                    .map(|(latitude, longitude)| Coordinates {
                        latitude: *latitude,
                        longitude: *longitude,
                    })
                    .collect(),
                identifier: Uuid::new_v4().to_string(),
                ..Default::default()
            }];

            let result = update_vertiports(vertiports).await.unwrap_err();
            assert_eq!(result, PostgisError::Vertiport(VertiportError::Location));
        }
    }

    #[test]
    fn test_vertiport_error_display() {
        let error = VertiportError::VertiportId;
        assert_eq!(error.to_string(), "Invalid vertiport ID provided.");

        let error = VertiportError::NoVertiports;
        assert_eq!(error.to_string(), "No vertiports were provided.");

        let error = VertiportError::Identifier;
        assert_eq!(error.to_string(), "Invalid label provided.");

        let error = VertiportError::Location;
        assert_eq!(error.to_string(), "Invalid vertices provided.");

        let error = VertiportError::Client;
        assert_eq!(error.to_string(), "Could not get backend client.");

        let error = VertiportError::DBError;
        assert_eq!(error.to_string(), "Unknown backend error.");

        let error = VertiportError::Timestamp;
        assert_eq!(error.to_string(), "Invalid timestamp provided.");
    }

    #[test]
    fn test_get_table_name() {
        assert_eq!(get_table_name(), r#""arrow"."vertiports""#);
    }
}
