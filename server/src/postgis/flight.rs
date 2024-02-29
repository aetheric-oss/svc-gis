//! This module contains functions for updating aircraft flight paths in the PostGIS database.

use super::{psql_transaction, PostgisError, DEFAULT_SRID, PSQL_SCHEMA};
use crate::cache::{Consumer, Processor};
use crate::postgis::utils::StringError;
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

/// Gets the name of the flights table
fn get_flights_table_name() -> &'static str {
    static FULL_NAME: &str = const_format::formatcp!(r#""{PSQL_SCHEMA}"."flights""#,);
    FULL_NAME
}
/// Gets the name of the flight segments table
fn get_flight_segments_table_name() -> &'static str {
    static FULL_NAME: &str = const_format::formatcp!(r#""{PSQL_SCHEMA}"."flight_segments""#,);
    FULL_NAME
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
            r#"CREATE TABLE IF NOT EXISTS {table_name} (
                "flight_identifier" VARCHAR(20) UNIQUE PRIMARY KEY NOT NULL,
                "aircraft_identifier" VARCHAR(20) NOT NULL,
                "aircraft_type" {enum_name} NOT NULL DEFAULT '{aircraft_type}',
                "simulated" BOOLEAN NOT NULL DEFAULT FALSE,
                "geom" GEOMETRY(LINESTRINGZ, {DEFAULT_SRID}), -- full path
                "isa" GEOMETRY NOT NULL, -- envelope
                "time_start" TIMESTAMPTZ,
                "time_end" TIMESTAMPTZ
            );"#,
            table_name = get_flights_table_name(),
            aircraft_type = AircraftType::Undeclared.to_string()
        ),
        format!(
            r#"CREATE TABLE IF NOT EXISTS {table_name} (
                "flight_identifier" VARCHAR(20) NOT NULL,
                "geom" GEOMETRY(LINESTRINGZ, {DEFAULT_SRID}),
                "time_start" TIMESTAMPTZ,
                "time_end" TIMESTAMPTZ,
                PRIMARY KEY ("flight_identifier", "time_start")
            );"#,
            table_name = get_flight_segments_table_name()
        ),
        format!(
            r#"CREATE INDEX IF NOT EXISTS "flights_geom_idx" ON {table_name} USING GIST ("isa");"#,
            table_name = get_flights_table_name()
        ),
        format!(
            r#"CREATE INDEX IF NOT EXISTS "flight_segments_geom_idx" ON {table_name} USING GIST (ST_Transform("geom", 4978));"#,
            table_name = get_flight_segments_table_name()
        ),
    ];

    psql_transaction(statements).await
}

#[async_trait]
impl Processor<FlightPath> for Consumer {
    async fn process(&mut self, items: Vec<FlightPath>) -> Result<(), ()> {
        if items.is_empty() {
            return Ok(());
        }

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

    let flights_insertion_stmt: String = format!(
        r#"INSERT INTO {table_name} (
            "flight_identifier",
            "aircraft_identifier",
            "aircraft_type",
            "simulated",
            "time_start",
            "time_end",
            "geom",
            "isa"
        )
        VALUES ($1, $2, $3, $4, $5, $6, $7, ST_Envelope($7))
        ON CONFLICT ("flight_identifier") DO UPDATE
            SET "aircraft_identifier" = EXCLUDED."aircraft_identifier",
                "aircraft_type" = EXCLUDED."aircraft_type",
                "simulated" = EXCLUDED."simulated",
                "geom" = EXCLUDED."geom",
                "isa" = EXCLUDED."isa",
                "time_start" = EXCLUDED."time_start",
                "time_end" = EXCLUDED."time_end";"#,
        table_name = get_flights_table_name()
    );

    let segments_deletion_stmt = format!(
        r#"DELETE FROM {table_name} WHERE "flight_identifier" = $1;"#,
        table_name = get_flight_segments_table_name()
    );

    let segment_insertion_stmt = format!(
        r#"INSERT INTO {table_name} (
            "flight_identifier",
            "geom",
            "time_start",
            "time_end"
        ) VALUES ( $1, $2, $3, $4 );"#,
        table_name = get_flight_segments_table_name()
    );

    let Some(pool) = crate::postgis::DEADPOOL_POSTGIS.get() else {
        postgis_error!("(update_flight_path) could not get psql pool.");
        return Err(PostgisError::FlightPath(FlightPathError::DBError));
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
        PostgisError::FlightPath(FlightPathError::Client)
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
            srid: Some(DEFAULT_SRID),
        };

        postgis_debug!("(update_flight_path) segmentizing path.");

        let Ok(segments) =
            super::utils::segmentize(points, flight.timestamp_start, flight.timestamp_end).await
        else {
            postgis_error!("(update_flight_path) could not segmentize path.");

            // continue to process other flights
            continue;
        };

        postgis_debug!("(update_flight_path) found segments: {:?}", segments);

        transaction
            .execute(
                &flights_insertion_stmt,
                &[
                    &flight.flight_identifier,
                    &flight.aircraft_identifier,
                    &flight.aircraft_type,
                    &flight.simulated,
                    &flight.timestamp_start,
                    &flight.timestamp_end,
                    &geom,
                ],
            )
            .await
            .map_err(|e| {
                postgis_error!(
                    "(update_flight_path) could not execute transaction to insert flight: {}",
                    e
                );
                PostgisError::FlightPath(FlightPathError::DBError)
            })?;

        transaction
            .execute(&segments_deletion_stmt, &[&flight.flight_identifier])
            .await
            .map_err(|e| {
                postgis_error!(
                    "(update_flight_path) could not execute transaction to delete segments: {}",
                    e
                );
                PostgisError::FlightPath(FlightPathError::DBError)
            })?;

        for segment in segments {
            transaction
                .execute(
                    &segment_insertion_stmt,
                    &[
                        &flight.flight_identifier,
                        &segment.geom,
                        &segment.time_start,
                        &segment.time_end,
                    ],
                )
                .await
                .map_err(|e| {
                    postgis_error!(
                        "(update_flight_path) could not execute transaction to insert segment: {}",
                        e
                    );
                    PostgisError::FlightPath(FlightPathError::DBError)
                })?;
        }
    }

    transaction.commit().await.map_err(|e| {
        postgis_error!("(update_flight_path) could not commit transaction: {}", e);
        PostgisError::FlightPath(FlightPathError::DBError)
    })?;

    postgis_info!("(update_flight_path) success.");
    Ok(())
}

/// Prepares a statement that checks zone intersections with the provided geometry
pub async fn get_flight_intersection_stmt(
    client: &Object,
) -> Result<tokio_postgres::Statement, PostgisError> {
    let result = client
        .prepare_cached(&format!(
            r#"WITH "segments" AS (
                SELECT
                    "flight_identifier",
                    "geom",
                    "time_start",
                    "time_end"
                FROM {segments_table_name}
                WHERE
                    ("time_start" <= $4 OR "time_start" IS NULL) -- easy checks first
                    AND ("time_end" >= $3 OR "time_end" IS NULL)
                    AND ST_3DDWithin(
                        ST_Transform("geom", 4978),
                        ST_Transform($1, 4978),
                        $2 -- meters
                    )
            ) SELECT
                "flight_identifier",
                "aircraft_identifier",
                "geom",
                "time_start",
                "time_end"
            FROM {flights_table_name}
            WHERE "flight_identifier" IN (SELECT "flight_identifier" FROM "segments")
                AND "simulated" = FALSE
            LIMIT 1;
        "#,
            segments_table_name = get_flight_segments_table_name(),
            flights_table_name = get_flights_table_name(),
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
    use chrono::{Duration, Utc};

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
        assert_eq!(result, PostgisError::FlightPath(FlightPathError::DBError));

        ut_info!("(ut_client_failure) success");
    }
}
