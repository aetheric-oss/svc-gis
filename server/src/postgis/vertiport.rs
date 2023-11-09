//! Updates vertiports in the PostGIS database.

use crate::grpc::server::grpc_server;
use grpc_server::Vertiport as RequestVertiport;
use uuid::Uuid;

/// Maximum length of a label
const LABEL_MAX_LENGTH: usize = 100;

/// Allowed characters in a label
const LABEL_REGEX: &str = r"^[a-zA-Z0-9_\s-]+$";

/// Possible conversion errors from the GRPC type to GIS type
#[derive(Debug, Copy, Clone, PartialEq)]
pub enum VertiportError {
    /// Invalid Vertiport ID
    VertiportId,

    /// No Vertiports
    NoVertiports,

    /// Invalid Label
    Label,

    /// Location of one or more vertices is invalid
    Location,

    /// Unknown error
    Unknown,
}

impl std::fmt::Display for VertiportError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            VertiportError::VertiportId => write!(f, "Invalid vertiport ID provided."),
            VertiportError::NoVertiports => write!(f, "No vertiports were provided."),
            VertiportError::Label => write!(f, "Invalid label provided."),
            VertiportError::Location => write!(f, "Invalid vertices provided."),
            VertiportError::Unknown => write!(f, "Unknown error."),
        }
    }
}

struct Vertiport {
    uuid: Uuid,
    label: Option<String>,
    geom: postgis::ewkb::Polygon,
}

impl TryFrom<RequestVertiport> for Vertiport {
    type Error = VertiportError;

    fn try_from(vertiport: RequestVertiport) -> Result<Self, Self::Error> {
        let Ok(uuid) = Uuid::parse_str(&vertiport.uuid) else {
            postgis_error!(
                "(try_from RequestVertiport) Invalid vertiport UUID: {}",
                vertiport.uuid
            );
            return Err(VertiportError::VertiportId);
        };

        let label = match &vertiport.label {
            Some(label) => {
                if let Err(e) = super::utils::check_string(label, LABEL_REGEX, LABEL_MAX_LENGTH) {
                    postgis_error!(
                        "(try_from RequestVertiport) Vertiport {} has invalid label: {}",
                        vertiport.uuid,
                        e
                    );
                    return Err(VertiportError::Label);
                }
                Some(label.to_string())
            }
            None => None,
        };

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

        Ok(Vertiport { uuid, label, geom })
    }
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

    let Ok(mut client) = pool.get().await else {
        postgis_error!("(update_vertiports) error getting client.");
        return Err(VertiportError::Unknown);
    };

    let Ok(transaction) = client.transaction().await else {
        postgis_error!("(update_vertiports) error creating transaction.");
        return Err(VertiportError::Unknown);
    };

    let Ok(stmt) = transaction
        .prepare_cached("SELECT arrow.update_vertiport($1, $2, $3)")
        .await
    else {
        postgis_error!("(update_vertiports) error preparing cached statement.");
        return Err(VertiportError::Unknown);
    };

    for vertiport in &vertiports {
        if let Err(e) = transaction
            .execute(&stmt, &[&vertiport.uuid, &vertiport.geom, &vertiport.label])
            .await
        {
            postgis_error!("(update_vertiports) error: {}", e);
            return Err(VertiportError::Unknown);
        }
    }

    match transaction.commit().await {
        Ok(_) => {
            postgis_debug!("(update_vertiports) success.");
        }
        Err(e) => {
            postgis_error!("(update_vertiports) error: {}", e);
            return Err(VertiportError::Unknown);
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

    fn square(latitude: f32, longitude: f32) -> Vec<(f32, f32)> {
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
                uuid: Uuid::new_v4().to_string(),
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
        let nodes: Vec<(&str, Vec<(f32, f32)>)> =
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
                uuid: Uuid::new_v4().to_string(),
            })
            .collect();

        let result = update_vertiports(vertiports, get_psql_pool().await)
            .await
            .unwrap_err();
        assert_eq!(result, VertiportError::Unknown);
    }

    #[tokio::test]
    async fn ut_vertiports_invalid_uuid() {
        let vertiports: Vec<RequestVertiport> = vec![RequestVertiport {
            uuid: "".to_string(),
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
            &"X".repeat(LABEL_MAX_LENGTH + 1),
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
                uuid: Uuid::new_v4().to_string(),
            }];

            let result = update_vertiports(vertiports, get_psql_pool().await)
                .await
                .unwrap_err();
            assert_eq!(result, VertiportError::Label);
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
                uuid: Uuid::new_v4().to_string(),
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
                uuid: Uuid::new_v4().to_string(),
                ..Default::default()
            }];

            let result = update_vertiports(vertiports, get_psql_pool().await)
                .await
                .unwrap_err();
            assert_eq!(result, VertiportError::Location);
        }
    }
}
