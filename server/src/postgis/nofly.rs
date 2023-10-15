//! This module contains functions for updating no-fly zones in the PostGIS database.
//! No-Fly Zones are permanent or temporary.

use crate::grpc::server::grpc_server;
use chrono::{DateTime, Utc};
use grpc_server::NoFlyZone as RequestNoFlyZone;

/// Maximum length of a label
const LABEL_MAX_LENGTH: usize = 100;

/// Allowed characters in a label
const LABEL_REGEX: &str = r"^[a-zA-Z0-9_\s-]+$";

#[derive(Clone, Debug)]
/// Nodes that aircraft can fly between
pub struct NoFlyZone {
    /// A unique identifier for the No-Fly Zone (NOTAM id, etc.)
    pub label: String,

    /// The geometry string to feed into PSQL
    pub geom: postgis::ewkb::Polygon,

    /// The start time of the no-fly zone, if applicable
    pub time_start: Option<DateTime<Utc>>,

    /// The end time of the no-fly zone, if applicable
    pub time_end: Option<DateTime<Utc>>,
}

/// Possible conversion errors from the GRPC type to GIS type
#[derive(Debug, Copy, Clone, PartialEq)]
pub enum NoFlyZoneError {
    /// Invalid timestamp format
    Time,

    /// End time earlier than start time
    TimeOrder,

    /// One or more vertices have an invalid location
    Location,

    /// Invalid Label
    Label,

    /// No No-Fly Zones
    NoZones,

    /// Unknown error
    Unknown,
}

impl std::fmt::Display for NoFlyZoneError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            NoFlyZoneError::Time => write!(f, "Invalid timestamp provided."),
            NoFlyZoneError::TimeOrder => write!(f, "Start time is later than end time."),
            NoFlyZoneError::NoZones => write!(f, "No No-Fly Zones were provided."),
            NoFlyZoneError::Location => write!(f, "Invalid location provided."),
            NoFlyZoneError::Unknown => write!(f, "Unknown error."),
            NoFlyZoneError::Label => write!(f, "Invalid label provided."),
        }
    }
}

impl TryFrom<RequestNoFlyZone> for NoFlyZone {
    type Error = NoFlyZoneError;

    fn try_from(zone: RequestNoFlyZone) -> Result<Self, Self::Error> {
        if let Err(e) = super::utils::check_string(&zone.label, LABEL_REGEX, LABEL_MAX_LENGTH) {
            postgis_error!(
                "(try_from RequestNoFlyZone) Invalid no-fly zone label: {}; {}",
                zone.label,
                e
            );
            return Err(NoFlyZoneError::Label);
        }

        let time_start = zone.time_start.map(Into::<DateTime<Utc>>::into);
        let time_end = zone.time_end.map(Into::<DateTime<Utc>>::into);

        // The start time must be earlier than the end time if both are provided
        if let Some(ts) = time_start {
            if let Some(te) = time_end {
                if te < ts {
                    postgis_error!(
                        "(try_from RequestNoFlyZone) end time is earlier than start time."
                    );
                    return Err(NoFlyZoneError::TimeOrder);
                }
            }
        }

        let geom = match super::utils::polygon_from_vertices(&zone.vertices) {
            Ok(geom) => geom,
            Err(e) => {
                postgis_error!(
                    "(try_from RequestNoFlyZone) Error converting nofly polygon: {}",
                    e.to_string()
                );
                return Err(NoFlyZoneError::Location);
            }
        };

        Ok(NoFlyZone {
            label: zone.label,
            geom,
            time_start,
            time_end,
        })
    }
}

/// Updates no-fly zones in the PostGIS database.
pub async fn update_nofly(
    zones: Vec<RequestNoFlyZone>,
    pool: deadpool_postgres::Pool,
) -> Result<(), NoFlyZoneError> {
    postgis_debug!("(postgis update_nofly) entry.");
    if zones.is_empty() {
        postgis_error!("(postgis update_nofly) no no-fly zones provided.");
        return Err(NoFlyZoneError::NoZones);
    }

    let zones: Vec<NoFlyZone> = zones
        .into_iter()
        .map(NoFlyZone::try_from)
        .collect::<Result<Vec<_>, _>>()?;

    let Ok(mut client) = pool.get().await else {
        postgis_error!("(postgis update_nofly) error getting client.");
        return Err(NoFlyZoneError::Unknown);
    };

    let Ok(transaction) = client.transaction().await else {
        postgis_error!("(postgis update_nofly) error creating transaction.");
        return Err(NoFlyZoneError::Unknown);
    };

    let Ok(stmt) = transaction
        .prepare_cached("SELECT arrow.update_nofly($1, $2, $3, $4)")
        .await
    else {
        postgis_error!("(postgis update_nofly) error preparing cached statement.");
        return Err(NoFlyZoneError::Unknown);
    };

    for zone in &zones {
        if let Err(e) = transaction
            .execute(
                &stmt,
                &[&zone.label, &zone.geom, &zone.time_start, &zone.time_end],
            )
            .await
        {
            postgis_error!("(postgis update_nofly) error: {}", e);
            return Err(NoFlyZoneError::Unknown);
        }
    }

    match transaction.commit().await {
        Ok(_) => {
            postgis_debug!("(postgis update_nofly) success.");
        }
        Err(e) => {
            postgis_error!("(postgis update_nofly) error: {}", e);
            return Err(NoFlyZoneError::Unknown);
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
            ("NFZ A", square(52.3745905, 4.9160036)),
            ("NFZ B", square(52.3749819, 4.9156925)),
            ("NFZ C", square(52.3752144, 4.9153733)),
        ];

        let nofly_zones: Vec<RequestNoFlyZone> = nodes
            .iter()
            .map(|(label, points)| RequestNoFlyZone {
                label: label.to_string(),
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

        let converted = nofly_zones
            .clone()
            .into_iter()
            .map(NoFlyZone::try_from)
            .collect::<Result<Vec<_>, _>>()
            .unwrap();

        assert_eq!(nofly_zones.len(), converted.len());

        for (i, nfz) in nofly_zones.iter().enumerate() {
            assert_eq!(nfz.label, converted[i].label);
            assert_eq!(
                utils::polygon_from_vertices(&nfz.vertices).unwrap(),
                converted[i].geom
            );
        }
    }

    #[tokio::test]
    async fn ut_client_failure() {
        let nodes: Vec<(&str, Vec<(f32, f32)>)> = vec![("NFZ", square(52.3745905, 4.9160036))];
        let nofly_zone: Vec<RequestNoFlyZone> = nodes
            .iter()
            .map(|(label, points)| RequestNoFlyZone {
                label: label.to_string(),
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

        let result = update_nofly(nofly_zone, get_pool()).await.unwrap_err();
        assert_eq!(result, NoFlyZoneError::Unknown);
    }

    #[tokio::test]
    async fn ut_nofly_request_to_gis_invalid_label() {
        for label in &[
            "NULL",
            "Nofly_zone;",
            "'Nofly_zone'",
            "Nofly_zone \'",
            &"X".repeat(LABEL_MAX_LENGTH + 1),
        ] {
            let nofly_zones: Vec<RequestNoFlyZone> = vec![RequestNoFlyZone {
                label: label.to_string(),
                vertices: square(52.3745905, 4.9160036)
                    .iter()
                    .map(|(latitude, longitude)| Coordinates {
                        latitude: *latitude,
                        longitude: *longitude,
                    })
                    .collect(),
                ..Default::default()
            }];

            let result = update_nofly(nofly_zones, get_pool()).await.unwrap_err();
            assert_eq!(result, NoFlyZoneError::Label);
        }
    }

    #[tokio::test]
    async fn ut_nofly_request_to_gis_invalid_no_nodes() {
        let nofly_zones: Vec<RequestNoFlyZone> = vec![];
        let result = update_nofly(nofly_zones, get_pool()).await.unwrap_err();
        assert_eq!(result, NoFlyZoneError::NoZones);
    }

    #[tokio::test]
    async fn ut_nofly_request_to_gis_invalid_location() {
        let polygons = vec![
            square(-90., 0.),
            square(90., 0.),
            square(0., -180.),
            square(0., 180.),
        ]; // each of these will crate a square outside of the allowable range of lat, lon

        for polygon in polygons {
            let nofly_zones: Vec<RequestNoFlyZone> = vec![RequestNoFlyZone {
                label: "Nofly_zone".to_string(),
                vertices: polygon
                    .iter()
                    .map(|(latitude, longitude)| Coordinates {
                        latitude: *latitude,
                        longitude: *longitude,
                    })
                    .collect(),
                ..Default::default()
            }];

            let result = update_nofly(nofly_zones, get_pool()).await.unwrap_err();
            assert_eq!(result, NoFlyZoneError::Location);
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
            let nofly_zones: Vec<RequestNoFlyZone> = vec![RequestNoFlyZone {
                label: "Nofly_zone".to_string(),
                vertices: polygon
                    .iter()
                    .map(|(latitude, longitude)| Coordinates {
                        latitude: *latitude,
                        longitude: *longitude,
                    })
                    .collect(),
                ..Default::default()
            }];

            let result = update_nofly(nofly_zones, get_pool()).await.unwrap_err();
            assert_eq!(result, NoFlyZoneError::Location);
        }
    }
}
