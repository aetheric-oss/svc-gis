//! This module contains functions for updating aircraft flight paths in the PostGIS database.

use super::psql_transaction;
use super::PostgisError;
use crate::postgis::utils::StringError;
use postgis::ewkb::PointZ;

use crate::types::{AircraftType, FlightPath};

/// Allowed characters in a identifier
pub const FLIGHT_IDENTIFIER_REGEX: &str = r"^[\-0-9A-Za-z_\.]{1,255}$";

/// Possible errors with aircraft requests
#[derive(Debug, Copy, Clone, PartialEq)]
pub enum FlightPathError {
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

impl std::fmt::Display for FlightPathError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            FlightPathError::AircraftId => write!(f, "Invalid aircraft ID provided."),
            FlightPathError::Location => write!(f, "Invalid location provided."),
            FlightPathError::Time => write!(f, "Invalid time provided."),
            FlightPathError::Label => write!(f, "Invalid label provided."),
            FlightPathError::Client => write!(f, "Could not get backend client."),
            FlightPathError::DBError => write!(f, "Unknown backend error."),
        }
    }
}

/// Verifies that a identifier is valid
pub fn check_flight_identifier(identifier: &str) -> Result<(), StringError> {
    super::utils::check_string(identifier, FLIGHT_IDENTIFIER_REGEX)
}

/// Initializes the PostGIS database for aircraft.
pub async fn psql_init() -> Result<(), PostgisError> {
    // Create Aircraft Table
    let table_name = "arrow.flights";
    let enum_name = "aircrafttype";
    let statements = vec![
        // super::psql_enum_declaration::<AircraftType>(enum_name), // should already exist
        format!(
            "CREATE TABLE IF NOT EXISTS {table_name} (
                flight_identifier VARCHAR(20) UNIQUE PRIMARY KEY NOT NULL,
                aircraft_identifier VARCHAR(20) NOT NULL,
                aircraft_type {enum_name} NOT NULL DEFAULT '{}',
                simulated BOOLEAN NOT NULL DEFAULT FALSE,
                path GEOMETRY(LINESTRINGZ, 4326),
                isa GEOMETRY NOT NULL,
                start TIMESTAMPTZ,
                end TIMESTAMPTZ,
            );",
            AircraftType::Undeclared.to_string()
        ),
    ];

    psql_transaction(statements).await
}

/// Validates the provided aircraft identification.
fn validate_flight_path(item: &FlightPath) -> Result<(), PostgisError> {
    if let Err(e) = check_flight_identifier(&item.flight_identifier) {
        postgis_error!(
            "(validate_flight_path) invalid identifier {}: {}",
            item.flight_identifier,
            e
        );

        return Err(PostgisError::FlightPath(FlightPathError::Label));
    }

    Ok(())
}

/// Pulls queued flight path messages from Redis Queue (from svc-scheduler)
pub async fn update_flight_path(flights: Vec<FlightPath>) -> Result<(), PostgisError> {
    postgis_debug!("(update_flight_path) entry.");
    if flights.is_empty() {
        return Ok(());
    }

    let Some(pool) = crate::postgis::DEADPOOL_POSTGIS.get() else {
        postgis_error!("(update_flight_path) could not get psql pool.");
        return Err(PostgisError::FlightPath(FlightPathError::Client));
    };

    let mut client = pool.get().await.map_err(|e| {
        postgis_error!(
            "(update_flight_path) could not get client from psql connection pool: {}",
            e
        );

        PostgisError::FlightPath(FlightPathError::Client)
    })?;

    let transaction = client.transaction().await.map_err(|e| {
        postgis_error!("(update_flight_path) could not create transaction: {}", e);

        PostgisError::FlightPath(FlightPathError::DBError)
    })?;

    let table_name = format!("{}.flights", super::PSQL_SCHEMA);
    let stmt = transaction
        .prepare_cached(&format!(
            "
        INSERT INTO {table_name}(
            flight_identifier,
            aircraft_identifier,
            aircraft_type,
            simulated,
            path,
            isa,
            start,
            end
        )
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
        ON CONFLICT (flight_identifier) DO UPDATE
            SET aircraft_identifier = EXCLUDED.aircraft_identifier,
                aircraft_type = EXCLUDED.aircraft_type,
                simulated = EXCLUDED.simulated,
                path = EXCLUDED.path,
                isa = EXCLUDED.isa,
                start = EXCLUDED.start,
                end = EXCLUDED.end
        ",
        ))
        .await
        .map_err(|e| {
            postgis_error!(
                "(update_flight_path) could not prepare cached statement: {}",
                e
            );
            PostgisError::FlightPath(FlightPathError::DBError)
        })?;

    for flight in &flights {
        if let Err(e) = validate_flight_path(flight) {
            postgis_error!(
                "(update_flight_path) could not validate id for flight id {}: {:?}",
                flight.flight_identifier,
                e
            );

            // TODO(R5): Should we actually toss these out?
            // Risk not registering a flight path on a clerical error and planning colliding paths
            continue;
        }

        let points = flight
            .path
            .clone()
            .into_iter()
            .map(PointZ::try_from)
            .collect::<Result<Vec<PointZ>, _>>()
            .map_err(|_| {
                postgis_error!("(update_flight_path) could not convert path to Vec<PointZ>.");

                PostgisError::FlightPath(FlightPathError::Location)
            })?;

        let path = postgis::ewkb::LineStringT {
            points,
            srid: Some(4326),
        };

        // Draw bounding box around flight for ISA
        // let isa = postgis::ewkb::Geometry::from(flight.isa.clone());
        let isa: i32 = 1;

        transaction
            .execute(
                &stmt,
                &[
                    &flight.flight_identifier,
                    &flight.aircraft_identifier,
                    &flight.aircraft_type,
                    &flight.simulated,
                    &path,
                    &isa,
                    &flight.timestamp_start,
                    &flight.timestamp_end,
                ],
            )
            .await
            .map_err(|e| {
                postgis_error!("(update_flight_path) could not execute transaction: {}", e);
                PostgisError::FlightPath(FlightPathError::DBError)
            })?;
    }

    match transaction.commit().await {
        Ok(_) => {
            postgis_debug!("(update_flight_path) success.");
            Ok(())
        }
        Err(e) => {
            postgis_error!("(update_flight_path) could not commit transaction: {}", e);
            Err(PostgisError::FlightPath(FlightPathError::DBError))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;

    #[tokio::test]
    async fn ut_client_failure() {
        crate::get_log_handle().await;
        ut_info!("(ut_client_failure) start");

        let nodes = vec![("aircraft", 52.3745905, 4.9160036)];
        let aircraft: Vec<AircraftPosition> = nodes
            .iter()
            .map(|(label, latitude, longitude)| AircraftPosition {
                identifier: label.to_string(),
                position: Position {
                    latitude: *latitude,
                    longitude: *longitude,
                    altitude_meters: 100.0,
                },
                timestamp_network: Utc::now(),
                timestamp_asset: None,
            })
            .collect();

        let result = update_aircraft_position(aircraft).await.unwrap_err();
        assert_eq!(result, PostgisError::FlightPath(FlightPathError::Client));

        ut_info!("(ut_client_failure) success");
    }
}
