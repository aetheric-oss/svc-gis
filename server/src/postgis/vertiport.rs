//! Updates vertiports in the PostGIS database.

use crate::grpc::server::grpc_server;
use grpc_server::Vertiport as RequestVertiport;
use grpc_server::ZoneType;

/// Maximum length of a label
const IDENTIFIER_MAX_LENGTH: usize = 255;

/// Allowed characters in a label
const IDENTIFIER_REGEX: &str = r"^[a-zA-Z0-9_\s-]+$";

/// Vertiport overhead no-fly clearance
const VERTIPORT_CLEARANCE_METERS: f64 = 200.0;

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
}

impl std::fmt::Display for VertiportError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            VertiportError::VertiportId => write!(f, "Invalid vertiport ID provided."),
            VertiportError::NoVertiports => write!(f, "No vertiports were provided."),
            VertiportError::Identifier => write!(f, "Invalid label provided."),
            VertiportError::Location => write!(f, "Invalid vertices provided."),
            VertiportError::Client => write!(f, "Could not get backend client."),
            VertiportError::DBError => write!(f, "Unknown backend error."),
        }
    }
}

/// Helper Struct for Validating Requests
struct Vertiport {
    identifier: String,
    label: Option<String>,
    geom: postgis::ewkb::Polygon,
    altitude_meters: f32,
}

impl TryFrom<RequestVertiport> for Vertiport {
    type Error = VertiportError;

    fn try_from(vertiport: RequestVertiport) -> Result<Self, Self::Error> {
        if let Err(e) = super::utils::check_string(
            &vertiport.identifier,
            IDENTIFIER_REGEX,
            IDENTIFIER_MAX_LENGTH,
        ) {
            postgis_error!(
                "(try_from RequestVertiport) Vertiport {} has invalid label {:?}: {}",
                vertiport.identifier,
                vertiport.label,
                e
            );
            return Err(VertiportError::Identifier);
        }

        let geom = match super::utils::polygon_from_vertices(&vertiport.vertices) {
            Ok(geom) => geom,
            Err(e) => {
                postgis_error!(
                    "(try_from RequestVertiport) Error converting vertiport polygon: {}",
                    e.to_string()
                );
                return Err(VertiportError::Location);
            }
        };

        // TODO(R4): Check altitude

        Ok(Vertiport {
            identifier: vertiport.identifier,
            label: vertiport.label,
            geom,
            altitude_meters: vertiport.altitude_meters,
        })
    }
}

/// Initialize the vertiports table in the PostGIS database
pub async fn psql_init(pool: &deadpool_postgres::Pool) -> Result<(), super::PsqlError> {
    // Create Aircraft Table
    let table_name = "arrow.vertiports";
    let statements = vec![format!(
        "CREATE TABLE IF NOT EXISTS {table_name} (
            identifier VARCHAR(255) NOT NULL UNIQUE PRIMARY KEY,
            label VARCHAR(255) NOT NULL,
            zone_id INTEGER NOT NULL,
            CONSTRAINT fk_zone
                FOREIGN KEY (zone_id)
                REFERENCES arrow.zones(id)
        );"
    )];

    super::psql_transaction(statements, pool).await
}

/// Update vertiports in the PostGIS database
pub async fn update_vertiports(
    vertiports: Vec<RequestVertiport>,
    pool: &deadpool_postgres::Pool,
) -> Result<(), VertiportError> {
    postgis_debug!("(update_vertiports) entry.");
    if vertiports.is_empty() {
        return Err(VertiportError::NoVertiports);
    }

    let vertiports: Vec<Vertiport> = vertiports
        .into_iter()
        .map(Vertiport::try_from)
        .collect::<Result<Vec<_>, _>>()?;

    let mut client = pool.get().await.map_err(|e| {
        postgis_error!(
            "(update_vertiports) could not get client from psql connection pool: {}",
            e
        );
        VertiportError::Client
    })?;

    let transaction = client.transaction().await.map_err(|e| {
        postgis_error!("(update_vertiports) could not create transaction: {}", e);
        VertiportError::DBError
    })?;

    let stmt = transaction
        .prepare_cached(&format!(
            "\
        BEGIN;
            DO $$
            DECLARE
                zid INTEGER;
            BEGIN
                INSERT INTO arrow.zones(
                    identifier,
                    geom,
                    altitude_meters_min,
                    altitude_meters_max
                )
                VALUES ($1, $2, $3, $3 + {VERTIPORT_CLEARANCE_METERS})
                    ON CONFLICT (identifier) DO UPDATE
                    SET
                        geom = $2,
                        zone_type = $5
                RETURNING id INTO zid;

                INSERT INTO arrow.vertiports(identifier, zone_id, label)
                VALUES (
                    $1,
                    zid,
                    $4
                ) ON CONFLICT (identifier) DO UPDATE (
                    IF $4 IS NOT NULL THEN
                        SET label = $4;
                    END IF;

                    SET zone_id = zid
                );
            END; $$;
        COMMIT;
        "
        ))
        .await
        .map_err(|e| {
            postgis_error!(
                "(update_vertiports) could not prepare cached statement: {}",
                e
            );
            VertiportError::DBError
        })?;

    for vertiport in &vertiports {
        transaction
            .execute(
                &stmt,
                &[
                    &vertiport.identifier,
                    &vertiport.geom,
                    &vertiport.altitude_meters,
                    &vertiport.label,
                    &ZoneType::Port,
                ],
            )
            .await
            .map_err(|e| {
                postgis_error!("(update_vertiports) could not execute transaction: {}", e);
                VertiportError::DBError
            })?;
    }

    match transaction.commit().await {
        Ok(_) => {
            postgis_debug!("(update_vertiports) success.");
            Ok(())
        }
        Err(e) => {
            postgis_error!("(update_vertiports) could not commit transaction: {}", e);
            Err(VertiportError::DBError)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::grpc::server::grpc_server::Coordinates;
    use crate::postgis::utils;
    use crate::test_util::get_psql_pool;
    use uuid::Uuid;

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
        let nodes = vec![
            ("Vertiport A", square(52.3745905, 4.9160036)),
            ("Vertiport B", square(52.3749819, 4.9156925)),
            ("Vertiport C", square(52.3752144, 4.9153733)),
        ];

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
                utils::polygon_from_vertices(&vertiport.vertices).unwrap(),
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
            })
            .collect();

        let result = update_vertiports(vertiports, get_psql_pool().await)
            .await
            .unwrap_err();
        assert_eq!(result, VertiportError::Client);
    }

    #[tokio::test]
    async fn ut_vertiports_invalid_uuid() {
        let vertiports: Vec<RequestVertiport> = vec![RequestVertiport {
            identifier: "".to_string(),
            vertices: square(52.3745905, 4.9160036)
                .iter()
                .map(|(latitude, longitude)| Coordinates {
                    latitude: *latitude,
                    longitude: *longitude,
                })
                .collect(),
            ..Default::default()
        }];

        let result = update_vertiports(vertiports, get_psql_pool().await)
            .await
            .unwrap_err();
        assert_eq!(result, VertiportError::VertiportId);
    }

    #[tokio::test]
    async fn ut_vertiports_request_to_gis_invalid_label() {
        for label in &[
            "NULL",
            "Vertiport;",
            "'Vertiport'",
            "Vertiport \'",
            &"X".repeat(IDENTIFIER_MAX_LENGTH + 1),
        ] {
            let vertiports: Vec<RequestVertiport> = vec![RequestVertiport {
                label: Some(label.to_string()),
                vertices: square(52.3745905, 4.9160036)
                    .iter()
                    .map(|(latitude, longitude)| Coordinates {
                        latitude: *latitude,
                        longitude: *longitude,
                    })
                    .collect(),
                identifier: Uuid::new_v4().to_string(),
                altitude_meters: 10.0,
            }];

            let result = update_vertiports(vertiports, get_psql_pool().await)
                .await
                .unwrap_err();
            assert_eq!(result, VertiportError::Identifier);
        }
    }

    #[tokio::test]
    async fn ut_vertiports_request_to_gis_invalid_no_nodes() {
        let vertiports: Vec<RequestVertiport> = vec![];
        let result = update_vertiports(vertiports, get_psql_pool().await)
            .await
            .unwrap_err();
        assert_eq!(result, VertiportError::NoVertiports);
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

            let result = update_vertiports(vertiports, get_psql_pool().await)
                .await
                .unwrap_err();
            assert_eq!(result, VertiportError::Location);
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

            let result = update_vertiports(vertiports, get_psql_pool().await)
                .await
                .unwrap_err();
            assert_eq!(result, VertiportError::Location);
        }
    }
}
