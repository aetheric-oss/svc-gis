//! This module contains functions for updating aircraft in the PostGIS database.

use crate::grpc::server::grpc_server;
use chrono::{DateTime, Utc};
use grpc_server::AircraftPosition as ReqAircraftPos;
use lib_common::time::timestamp_to_datetime;
use uuid::Uuid;

use super::utils::StringError;

/// Maximum length of a callsign
const LABEL_MAX_LENGTH: usize = 100;

/// Allowed characters in a callsign
const CALLSIGN_REGEX: &str = r"^[a-zA-Z0-9_\s-]+$";

/// Possible errors with aircraft requests
#[derive(Debug, Copy, Clone, PartialEq)]
pub enum AircraftError {
    /// No aircraft were provided
    NoAircraft,

    /// Invalid Aircraft ID
    AircraftId,

    /// Invalid Location
    Location,

    /// Invalid Time Provided
    Time,

    /// Invalid Label
    Label,

    /// Unknown error
    Unknown,
}

impl std::fmt::Display for AircraftError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            AircraftError::NoAircraft => write!(f, "No aircraft were provided."),
            AircraftError::AircraftId => write!(f, "Invalid aircraft ID provided."),
            AircraftError::Label => write!(f, "Invalid label provided."),
            AircraftError::Location => write!(f, "Invalid location provided."),
            AircraftError::Time => write!(f, "Invalid time provided."),
            AircraftError::Unknown => write!(f, "Unknown error."),
        }
    }
}

struct AircraftPosition {
    uuid: Option<Uuid>,
    callsign: String,
    geom: postgis::ewkb::Point,
    altitude_meters: f32,
    time: DateTime<Utc>,
}

/// Verifies that a callsign is valid
pub fn check_callsign(callsign: &str) -> Result<(), StringError> {
    super::utils::check_string(callsign, CALLSIGN_REGEX, LABEL_MAX_LENGTH)
}

impl TryFrom<ReqAircraftPos> for AircraftPosition {
    type Error = AircraftError;

    fn try_from(craft: ReqAircraftPos) -> Result<Self, Self::Error> {
        if let Err(e) = check_callsign(&craft.callsign) {
            postgis_error!(
                "(try_from ReqAircraftPos) Invalid aircraft callsign: {}; {}",
                craft.callsign,
                e
            );
            return Err(AircraftError::Label);
        }

        let uuid = match craft.uuid {
            Some(uuid) => match Uuid::parse_str(&uuid) {
                Err(e) => {
                    postgis_error!("(try_from ReqAircraftPos) Invalid aircraft UUID: {}", e);
                    return Err(AircraftError::AircraftId);
                }
                Ok(uuid) => Some(uuid),
            },
            None => None,
        };

        let Some(location) = craft.location else {
            postgis_error!(
                "(try_from ReqAircraftPos) Aircraft location is invalid."
            );
            return Err(AircraftError::Location);
        };

        let geom = match super::utils::point_from_vertex(&location) {
            Ok(geom) => geom,
            Err(e) => {
                postgis_error!(
                    "(try_from ReqAircraftPos) Error creating point from vertex: {}",
                    e
                );
                return Err(AircraftError::Location);
            }
        };

        let Some(time) = craft.time else {
            postgis_error!(
                "(try_from ReqAircraftPos) Aircraft time is invalid."
            );
            return Err(AircraftError::Time);
        };

        let Some(time) = timestamp_to_datetime(&time) else {
            postgis_error!(
                "(try_from ReqAircraftPos) Error converting timestamp to datetime."
            );
            return Err(AircraftError::Time);
        };

        Ok(AircraftPosition {
            uuid,
            callsign: craft.callsign,
            geom,
            altitude_meters: craft.altitude_meters,
            time,
        })
    }
}

/// Updates aircraft in the PostGIS database.
pub async fn update_aircraft_position(
    aircraft: Vec<ReqAircraftPos>,
    pool: deadpool_postgres::Pool,
) -> Result<(), AircraftError> {
    postgis_debug!("(postgis update_aircraft_position) entry.");
    if aircraft.is_empty() {
        return Err(AircraftError::NoAircraft);
    }

    let aircraft: Vec<AircraftPosition> = aircraft
        .into_iter()
        .map(AircraftPosition::try_from)
        .collect::<Result<Vec<_>, _>>()?;

    let Ok(mut client) = pool.get().await else {
        postgis_error!("(postgis update_aircraft_position) error getting client.");
        return Err(AircraftError::Unknown);
    };

    let Ok(transaction) = client.transaction().await else {
        postgis_error!("(postgis update_aircraft_position) error creating transaction.");
        return Err(AircraftError::Unknown);
    };

    let Ok(stmt) = transaction.prepare_cached(
        "SELECT arrow.update_aircraft_position($1, $2, $3, $4, $5::TIMESTAMPTZ)"
    ).await else {
        postgis_error!("(postgis update_aircraft_position) error preparing cached statement.");
        return Err(AircraftError::Unknown);
    };

    for craft in &aircraft {
        if let Err(e) = transaction
            .execute(
                &stmt,
                &[
                    &craft.uuid,
                    &craft.geom,
                    &(craft.altitude_meters as f64),
                    &craft.callsign,
                    &craft.time,
                ],
            )
            .await
        {
            postgis_error!("(postgis update_aircraft_position) error: {}", e);
            return Err(AircraftError::Unknown);
        }
    }

    match transaction.commit().await {
        Ok(_) => {
            postgis_debug!("(postgis update_aircraft_position) success.");
        }
        Err(e) => {
            postgis_error!("(postgis update_aircraft_position) error: {}", e);
            return Err(AircraftError::Unknown);
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::grpc::server::grpc_server::Coordinates;
    use crate::postgis::utils;
    use chrono::Utc;
    use deadpool_postgres::{Config, ManagerConfig, Pool, RecyclingMethod, Runtime};
    use lib_common::time::datetime_to_timestamp;
    use rand::{thread_rng, Rng};
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
    fn ut_request_valid() {
        let mut rng = thread_rng();
        let nodes = vec![
            ("Marauder", 52.3745905, 4.9160036),
            ("Phantom", 52.3749819, 4.9156925),
            ("Ghost", 52.3752144, 4.9153733),
            ("Falcon", 52.3753012, 4.9156845),
            ("Mantis", 52.3750703, 4.9161538),
        ];

        let aircraft: Vec<ReqAircraftPos> = nodes
            .iter()
            .map(|(label, latitude, longitude)| ReqAircraftPos {
                callsign: label.to_string(),
                location: Some(Coordinates {
                    latitude: *latitude,
                    longitude: *longitude,
                }),
                altitude_meters: rng.gen_range(0.0..2000.),
                uuid: Some(Uuid::new_v4().to_string()),
                time: datetime_to_timestamp(&Utc::now()),
            })
            .collect();

        let converted = aircraft
            .clone()
            .into_iter()
            .map(AircraftPosition::try_from)
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        assert_eq!(aircraft.len(), converted.len());

        for (i, aircraft) in aircraft.iter().enumerate() {
            assert_eq!(aircraft.callsign, converted[i].callsign);
            let location = aircraft.location.unwrap();
            assert_eq!(
                utils::point_from_vertex(&location).unwrap(),
                converted[i].geom
            );

            assert_eq!(aircraft.altitude_meters, converted[i].altitude_meters);

            if let Some(uuid) = aircraft.uuid.clone() {
                assert_eq!(
                    uuid,
                    converted[i].uuid.expect("Expected Some uuid.").to_string()
                );
            } else {
                assert_eq!(converted[i].uuid, None);
            }

            let time = aircraft.time.clone().expect("Expected Some time.");
            let converted = datetime_to_timestamp(&converted[i].time)
                .expect("Couldn't convert datetime to timestamp.");

            assert_eq!(time, converted);
        }
    }

    #[tokio::test]
    async fn ut_client_failure() {
        let nodes = vec![("aircraft", 52.3745905, 4.9160036)];
        let aircraft: Vec<ReqAircraftPos> = nodes
            .iter()
            .map(|(label, latitude, longitude)| ReqAircraftPos {
                callsign: label.to_string(),
                location: Some(Coordinates {
                    latitude: *latitude,
                    longitude: *longitude,
                }),
                uuid: Some(Uuid::new_v4().to_string()),
                time: datetime_to_timestamp(&Utc::now()),
                ..Default::default()
            })
            .collect();

        let result = update_aircraft_position(aircraft, get_pool())
            .await
            .unwrap_err();
        assert_eq!(result, AircraftError::Unknown);
    }

    #[tokio::test]
    async fn ut_aircraft_request_to_gis_invalid_label() {
        for label in &[
            "NULL",
            "Aircraft;",
            "'Aircraft'",
            "Aircraft \'",
            &"X".repeat(LABEL_MAX_LENGTH + 1),
        ] {
            let aircraft: Vec<ReqAircraftPos> = vec![ReqAircraftPos {
                callsign: label.to_string(),
                uuid: Some(Uuid::new_v4().to_string()),
                ..Default::default()
            }];

            let result = update_aircraft_position(aircraft, get_pool())
                .await
                .unwrap_err();
            assert_eq!(result, AircraftError::Label);
        }
    }

    #[tokio::test]
    async fn ut_aircraft_request_to_gis_invalid_no_nodes() {
        let aircraft: Vec<ReqAircraftPos> = vec![];
        let result = update_aircraft_position(aircraft, get_pool())
            .await
            .unwrap_err();
        assert_eq!(result, AircraftError::NoAircraft);
    }

    #[tokio::test]
    async fn ut_aircraft_request_to_gis_invalid_location() {
        let coords = vec![(-90.1, 0.0), (90.1, 0.0), (0.0, -180.1), (0.0, 180.1)];
        for coord in coords {
            let aircraft: Vec<ReqAircraftPos> = vec![ReqAircraftPos {
                location: Some(Coordinates {
                    latitude: coord.0,
                    longitude: coord.1,
                }),
                callsign: "Aircraft".to_string(),
                ..Default::default()
            }];

            let result = update_aircraft_position(aircraft, get_pool())
                .await
                .unwrap_err();
            assert_eq!(result, AircraftError::Location);
        }

        // No location
        let aircraft: Vec<ReqAircraftPos> = vec![ReqAircraftPos {
            location: None,
            callsign: "Aircraft".to_string(),
            ..Default::default()
        }];

        let result = update_aircraft_position(aircraft, get_pool())
            .await
            .unwrap_err();
        assert_eq!(result, AircraftError::Location);
    }

    #[tokio::test]
    async fn ut_aircraft_request_to_gis_invalid_time() {
        // No location
        let aircraft: Vec<ReqAircraftPos> = vec![ReqAircraftPos {
            time: None,
            location: Some(Coordinates {
                latitude: 0.0,
                longitude: 0.0,
            }),
            callsign: "Aircraft".to_string(),
            ..Default::default()
        }];

        let result = update_aircraft_position(aircraft, get_pool())
            .await
            .unwrap_err();
        assert_eq!(result, AircraftError::Time);
    }
}
