//! This module contains functions for updating aircraft in the PostGIS database.

use crate::grpc::server::grpc_server;
use chrono::{DateTime, Utc};
use grpc_server::AircraftPosition as ReqAircraftPos;
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

    /// Could not get client
    Client,

    /// DBError error
    DBError,
}

impl std::fmt::Display for AircraftError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            AircraftError::NoAircraft => write!(f, "No aircraft were provided."),
            AircraftError::AircraftId => write!(f, "Invalid aircraft ID provided."),
            AircraftError::Location => write!(f, "Invalid location provided."),
            AircraftError::Time => write!(f, "Invalid time provided."),
            AircraftError::Label => write!(f, "Invalid label provided."),
            AircraftError::Client => write!(f, "Could not get backend client."),
            AircraftError::DBError => write!(f, "Unknown backend error."),
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
            postgis_error!("(try_from ReqAircraftPos) Aircraft location is invalid.");
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
            postgis_error!("(try_from ReqAircraftPos) Aircraft time is invalid.");
            return Err(AircraftError::Time);
        };

        let time: DateTime<Utc> = time.into();

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
    pool: &deadpool_postgres::Pool,
) -> Result<(), AircraftError> {
    postgis_debug!("(update_aircraft_position) entry.");
    if aircraft.is_empty() {
        return Err(AircraftError::NoAircraft);
    }

    let aircraft: Vec<AircraftPosition> = aircraft
        .into_iter()
        .map(AircraftPosition::try_from)
        .collect::<Result<Vec<_>, _>>()?;
    let mut client = pool.get().await.map_err(|e| {
        postgis_error!(
            "(update_aircraft_position) could not get client from psql connection pool: {}",
            e
        );
        AircraftError::Client
    })?;
    let transaction = client.transaction().await.map_err(|e| {
        postgis_error!(
            "(update_aircraft_position) could not create transaction: {}",
            e
        );
        AircraftError::DBError
    })?;

    let stmt = transaction
        .prepare_cached("SELECT arrow.update_aircraft_position($1, $2, $3, $4, $5::TIMESTAMPTZ)")
        .await
        .map_err(|e| {
            postgis_error!(
                "(update_aircraft_position) could not prepare cached statement: {}",
                e
            );
            AircraftError::DBError
        })?;

    for craft in &aircraft {
        transaction
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
            .map_err(|e| {
                postgis_error!(
                    "(update_aircraft_position) could not execute transaction: {}",
                    e
                );
                AircraftError::DBError
            })?;
    }

    match transaction.commit().await {
        Ok(_) => {
            postgis_debug!("(update_aircraft_position) success.");
            Ok(())
        }
        Err(e) => {
            postgis_error!(
                "(update_aircraft_position) could not commit transaction: {}",
                e
            );
            Err(AircraftError::DBError)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::grpc::server::grpc_server::Coordinates;
    use crate::postgis::utils;
    use crate::test_util::get_psql_pool;
    use lib_common::time::*;
    use rand::{thread_rng, Rng};

    #[tokio::test]
    async fn ut_request_valid() {
        crate::get_log_handle().await;
        ut_info!("(ut_request_valid) start");

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
                time: Some(Into::<Timestamp>::into(Utc::now())),
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

            let time: Timestamp = aircraft.time.clone().expect("Expected Some time.");
            let converted: Timestamp = converted[i].time.into();

            assert_eq!(time, converted);
        }

        ut_info!("(ut_request_valid) success");
    }

    #[tokio::test]
    async fn ut_client_failure() {
        crate::get_log_handle().await;
        ut_info!("(ut_client_failure) start");

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
                time: Some(Into::<Timestamp>::into(Utc::now())),
                ..Default::default()
            })
            .collect();

        let result = update_aircraft_position(aircraft, get_psql_pool().await)
            .await
            .unwrap_err();
        assert_eq!(result, AircraftError::Client);

        ut_info!("(ut_client_failure) success");
    }

    #[tokio::test]
    async fn ut_aircraft_request_to_gis_invalid_label() {
        crate::get_log_handle().await;
        ut_info!("(ut_aircraft_request_to_gis_invalid_label) start");

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

            let result = update_aircraft_position(aircraft, get_psql_pool().await)
                .await
                .unwrap_err();
            assert_eq!(result, AircraftError::Label);
        }

        ut_info!("(ut_aircraft_request_to_gis_invalid_label) success");
    }

    #[tokio::test]
    async fn ut_aircraft_request_to_gis_invalid_no_nodes() {
        crate::get_log_handle().await;
        ut_info!("(ut_aircraft_request_to_gis_invalid_no_nodes) start");

        let aircraft: Vec<ReqAircraftPos> = vec![];
        let result = update_aircraft_position(aircraft, get_psql_pool().await)
            .await
            .unwrap_err();
        assert_eq!(result, AircraftError::NoAircraft);

        ut_info!("(ut_aircraft_request_to_gis_invalid_no_nodes) success");
    }

    #[tokio::test]
    async fn ut_aircraft_request_to_gis_invalid_location() {
        crate::get_log_handle().await;
        ut_info!("(ut_aircraft_request_to_gis_invalid_location) start");

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

            let result = update_aircraft_position(aircraft, get_psql_pool().await)
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

        let result = update_aircraft_position(aircraft, get_psql_pool().await)
            .await
            .unwrap_err();
        assert_eq!(result, AircraftError::Location);

        ut_info!("(ut_aircraft_request_to_gis_invalid_location) success");
    }

    #[tokio::test]
    async fn ut_aircraft_request_to_gis_invalid_time() {
        crate::get_log_handle().await;
        ut_info!("(ut_aircraft_request_to_gis_invalid_time) start");

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

        let result = update_aircraft_position(aircraft, get_psql_pool().await)
            .await
            .unwrap_err();
        assert_eq!(result, AircraftError::Time);

        ut_info!("(ut_aircraft_request_to_gis_invalid_time) success");
    }
}
