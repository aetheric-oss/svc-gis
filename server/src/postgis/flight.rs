//! This module contains functions for updating aircraft flight paths in the PostGIS database.

use super::psql_transaction;
use super::PostgisError;
use crate::cache::{Consumer, Processor};
use crate::postgis::utils::StringError;
use chrono::{Duration, Utc};
use deadpool_postgres::Object;
use postgis::ewkb::{LineStringT, PointZ};
use tonic::async_trait;

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
    let enum_name = "aircrafttype";
    let statements = vec![
        // super::psql_enum_declaration::<AircraftType>(enum_name), // should already exist
        format!(
            "CREATE TABLE IF NOT EXISTS {schema}.flights (
                flight_identifier VARCHAR(20) UNIQUE PRIMARY KEY NOT NULL,
                aircraft_identifier VARCHAR(20) NOT NULL,
                aircraft_type {enum_name} NOT NULL DEFAULT '{aircraft_type}',
                simulated BOOLEAN NOT NULL DEFAULT FALSE,
                geom GEOMETRY(LINESTRINGZ, 4326), -- full path
                isa GEOMETRY NOT NULL, -- envelope
                time_start TIMESTAMPTZ,
                time_end TIMESTAMPTZ
            );
            
            CREATE TABLE IF NOT EXISTS {schema}.flight_segments (
                flight_identifier VARCHAR(20) NOT NULL,
                geom GEOMETRY(LINESTRINGZ, 4326),
                time_start TIMESTAMPTZ,
                time_end TIMESTAMPTZ,
                PRIMARY KEY (flight_identifier, time_start)
            );
            
            CREATE INDEX IF NOT EXISTS flights_geom_idx ON {schema}.flights USING GIST (isa);
            CREATE INDEX IF NOT EXISTS flight_paths_idx ON {schema}.flight_segments USING GIST (geom);
            ",
            schema = super::PSQL_SCHEMA,
            aircraft_type = AircraftType::Undeclared.to_string()
        ),
    ];

    psql_transaction(statements).await
}

/// Starts a thread that will remove old flights from the PostGIS flights table
pub async fn psql_maintenance() -> Result<(), PostgisError> {
    postgis_info!("(flight::psql_maintenance) start.");
    let Some(pool) = crate::postgis::DEADPOOL_POSTGIS.get() else {
        postgis_error!("(psql_maintenance) could not get psql pool.");
        return Err(PostgisError::FlightPath(FlightPathError::Client));
    };

    // TODO(R5): These should be config settings
    const DURATION_H: i64 = 6;
    const FREQUENCY_S: u64 = 60;

    // Remove flights from postgis that are older than N hours
    let stmt = format!(
        "DELETE FROM {schema}.flight_segments WHERE time_end < $1;
        DELETE FROM {schema}.flights WHERE time_end < $1;",
        schema = super::PSQL_SCHEMA
    );

    tokio::spawn(async move {
        loop {
            postgis_info!("(flight::psql_maintenance) running scheduled maintenance.");

            match pool.get().await {
                Ok(client) => {
                    match client
                        .query(&stmt, &[&(Utc::now() - Duration::hours(DURATION_H))])
                        .await
                    {
                        Ok(result) => {
                            postgis_info!(
                                "(flight::psql_maintenance) removed {} flights older than {} hours.",
                                result.len(),
                                DURATION_H
                            );
                        }
                        Err(e) => {
                            postgis_error!(
                                "(flight::psql_maintenance) could not execute simple query: {}",
                                e
                            );
                        }
                    }
                }
                Err(e) => {
                    postgis_error!(
                        "(flight::psql_maintenance) could not get client from psql connection pool: {}",
                        e
                    );
                }
            };

            tokio::time::sleep(tokio::time::Duration::from_secs(FREQUENCY_S)).await;
        }
    });

    postgis_info!("(flight::psql_maintenance) successfully launched thread.");
    Ok(())
}

#[async_trait]
impl Processor<FlightPath> for Consumer {
    async fn process(&mut self, items: Vec<FlightPath>) -> Result<(), ()> {
        update_flight_path(items).await.map_err(|_| ())
    }
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

    let stmt = transaction
        .prepare_cached(&format!(
            "

        BEGIN;

        INSERT INTO {schema}.flights (
            flight_identifier,
            aircraft_identifier,
            aircraft_type,
            simulated,
            time_start,
            time_end,
            geom,
            isa
        )
        VALUES ($1, $2, $3, $4, $5, $6, $7, ST_Envelope($7))
        ON CONFLICT (flight_identifier) DO UPDATE
            SET aircraft_identifier = EXCLUDED.aircraft_identifier,
                aircraft_type = EXCLUDED.aircraft_type,
                simulated = EXCLUDED.simulated,
                geom = EXCLUDED.geom,
                isa = (ST_Envelope(EXCLUDED.geom)),
                time_start = EXCLUDED.time_start,
                time_end = EXCLUDED.time_end;

        DELETE FROM {schema}.flight_segments WHERE flight_identifier = $1;

        FOR row IN $8
        LOOP
            INSERT INTO {schema}.flight_segments (
                flight_identifier,
                geom,
                time_start,
                time_end
            )
            VALUES ($1, row.geom, row.time_start, row.time_end)
            ON CONFLICT (flight_identifier, time_start) DO UPDATE
                SET geom = EXCLUDED.geom,
                    time_end = EXCLUDED.time_end;
        END LOOP;
        
        COMMIT;
        ",
            schema = super::PSQL_SCHEMA
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

        // Subdivide the path into segments by length
        let geom = LineStringT {
            points: points.clone(),
            srid: Some(4326),
        };

        let Ok(segments) =
            super::utils::segmentize(points, flight.timestamp_start, flight.timestamp_end).await
        else {
            postgis_error!("(update_flight_path) could not segmentize path.");

            // continue to process other flights
            continue;
        };

        transaction
            .execute(
                &stmt,
                &[
                    &flight.flight_identifier,
                    &flight.aircraft_identifier,
                    &flight.aircraft_type,
                    &flight.simulated,
                    &flight.timestamp_start,
                    &flight.timestamp_end,
                    &geom,
                    &segments,
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

/// Prepares a statement that checks zone intersections with the provided geometry
pub async fn get_flight_intersection_stmt(
    client: &Object,
) -> Result<tokio_postgres::Statement, PostgisError> {
    let result = client
        .prepare_cached(&format!(
            "
            SELECT (
                flight_identifier,
                aircraft_identifier,
                geom,
                time_start,
                time_end
            )
            FROM {schema}.flights
            WHERE
                ST_DWithin(geom, $1::GEOMETRY, $2)
                AND (time_start <= $4 OR time_start IS NULL)
                AND (time_end >= $3 OR time_end IS NULL)
                AND (NOT simulated)
            LIMIT 1;
        ",
            schema = super::PSQL_SCHEMA,
        ))
        .await;

    match result {
        Ok(stmt) => Ok(stmt),
        Err(e) => {
            postgis_error!(
                "(get_flight_intersection_stmt) could not prepare cached statement: {}",
                e
            );
            Err(PostgisError::FlightPath(FlightPathError::DBError))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn ut_client_failure() {
        crate::get_log_handle().await;
        ut_info!("(ut_client_failure) start");

        let item = FlightPath {
            flight_identifier: "test".to_string(),
            aircraft_identifier: "test".to_string(),
            aircraft_type: AircraftType::Aeroplane,
            simulated: false,
            timestamp_start: Utc::now(),
            timestamp_end: Utc::now() + Duration::hours(1),
            path: vec![],
        };

        let result = update_flight_path(vec![item]).await.unwrap_err();
        assert_eq!(result, PostgisError::FlightPath(FlightPathError::Client));

        ut_info!("(ut_client_failure) success");
    }
}
