//! This module contains functions for updating aircraft flight paths in the PostGIS database.

use super::{psql_transaction, PostgisError, DEFAULT_SRID, PSQL_SCHEMA};
use crate::grpc::server::grpc_server::{
    AircraftState, Flight, GetFlightsRequest, PointZ as GrpcPointZ, TimePosition,
    UpdateFlightPathRequest,
};
use crate::postgis::utils::Segment;
use crate::postgis::utils::StringError;
use crate::types::AircraftType;
use crate::types::OperationalStatus;
use deadpool_postgres::Object;
use lib_common::time::{DateTime, Utc};
use num_traits::FromPrimitive;
use postgis::ewkb::{LineStringT, Point, PointZ};
use std::fmt::{self, Display, Formatter};

/// Allowed characters in a identifier
pub const FLIGHT_IDENTIFIER_REGEX: &str = r"^[\-0-9A-Za-z_\.]{1,255}$";

/// Max length of each flight segment in meters
pub const MAX_FLIGHT_SEGMENT_LENGTH_METERS: f32 = 40.0;

/// Possible errors with aircraft requests
#[derive(Debug, Copy, Clone, PartialEq)]
pub enum FlightError {
    /// Invalid Aircraft ID
    AircraftId,

    /// Invalid Aircraft Type
    AircraftType,

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

    /// Segmentize Error
    Segments,

    /// Intersection of flight segments
    Intersection,
}

impl Display for FlightError {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        match self {
            FlightError::AircraftId => write!(f, "Invalid aircraft ID provided."),
            FlightError::AircraftType => write!(f, "Invalid aircraft type provided."),
            FlightError::Location => write!(f, "Invalid location provided."),
            FlightError::Time => write!(f, "Invalid time provided."),
            FlightError::Label => write!(f, "Invalid label provided."),
            FlightError::Client => write!(f, "Could not get backend client."),
            FlightError::DBError => write!(f, "Unknown backend error."),
            FlightError::Segments => write!(f, "Could not segmentize path."),
            FlightError::Intersection => write!(f, "Flight paths intersect."),
        }
    }
}

/// Gets the name of the flights table
fn get_flights_table_name() -> &'static str {
    static FULL_NAME: &str = const_format::formatcp!(r#""{PSQL_SCHEMA}"."flights""#,);
    FULL_NAME
}

/// Verifies that a identifier is valid
pub fn check_flight_identifier(identifier: &str) -> Result<(), StringError> {
    super::utils::check_string(identifier, FLIGHT_IDENTIFIER_REGEX)
}

/// Initializes the PostGIS database for aircraft.
#[cfg(not(tarpaulin_include))]
// no_coverage: (R5) need psql backend to test, use integration tests
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
            r#"CREATE INDEX IF NOT EXISTS "flights_geom_idx" ON {table_name} USING GIST (ST_Transform("geom", 4978));"#,
            table_name = get_flights_table_name()
        ),
        format!(
            r#"CREATE INDEX IF NOT EXISTS "flights_isa_idx" ON {table_name} USING GIST ("isa");"#,
            table_name = get_flights_table_name()
        ),
    ];

    psql_transaction(statements).await
}

/// Validates the provided aircraft identification.
fn validate_flight_identifier(id: &Option<String>) -> Result<(), PostgisError> {
    let identifier = id.as_ref().ok_or_else(|| {
        postgis_error!("no identifier provided.");
        PostgisError::FlightPath(FlightError::Label)
    })?;

    check_flight_identifier(identifier).map_err(|e| {
        postgis_error!("invalid identifier {}: {}", identifier, e);
        PostgisError::FlightPath(FlightError::Label)
    })?;

    Ok(())
}

/// Pulls queued flight path messages from Redis Queue (from svc-scheduler)
#[cfg(not(tarpaulin_include))]
// no_coverage: (R5) need psql backend to test
pub async fn update_flight_path(flight: UpdateFlightPathRequest) -> Result<(), PostgisError> {
    postgis_debug!("entry.");

    validate_flight_identifier(&flight.flight_identifier).map_err(|e| {
        postgis_error!(
            "could not validate id for flight id {:?}: {:?}",
            flight.flight_identifier,
            e
        );

        e
    })?;

    let timestamp_start = flight.timestamp_start.ok_or_else(|| {
        postgis_error!("no start time provided.");
        PostgisError::FlightPath(FlightError::Time)
    })?;

    let timestamp_end = flight.timestamp_end.ok_or_else(|| {
        postgis_error!("no end time provided.");
        PostgisError::FlightPath(FlightError::Time)
    })?;

    let timestamp_start: DateTime<Utc> = timestamp_start.into();
    let timestamp_end: DateTime<Utc> = timestamp_end.into();
    let aircraft_type: AircraftType =
        FromPrimitive::from_i32(flight.aircraft_type).ok_or_else(|| {
            postgis_error!("invalid aircraft type provided.");
            PostgisError::FlightPath(FlightError::AircraftType)
        })?;

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

    let mut client = crate::postgis::DEADPOOL_POSTGIS
        .get()
        .ok_or_else(|| {
            postgis_error!("could not get psql pool.");
            PostgisError::FlightPath(FlightError::DBError)
        })?
        .get()
        .await
        .map_err(|e| {
            postgis_error!("could not get client from psql connection pool: {}", e);
            PostgisError::FlightPath(FlightError::Client)
        })?;

    let transaction = client.transaction().await.map_err(|e| {
        postgis_error!("could not create transaction: {}", e);
        PostgisError::FlightPath(FlightError::Client)
    })?;

    let points = flight
        .path
        .clone()
        .into_iter()
        .map(PointZ::try_from)
        .collect::<Result<Vec<PointZ>, _>>()
        .map_err(|_| {
            postgis_error!("could not convert path to Vec<PointZ>.");
            PostgisError::FlightPath(FlightError::Location)
        })?;

    // Subdivide the path into segments by length
    let geom = LineStringT {
        points,
        srid: Some(DEFAULT_SRID),
    };

    // postgis_debug!("found segments: {:?}", segments);

    transaction
        .execute(
            &flights_insertion_stmt,
            &[
                &flight.flight_identifier,
                &flight.aircraft_identifier,
                &aircraft_type,
                &flight.simulated,
                &timestamp_start,
                &timestamp_end,
                &geom,
            ],
        )
        .await
        .map_err(|e| {
            postgis_error!("could not execute transaction to insert flight: {}", e);
            PostgisError::FlightPath(FlightError::DBError)
        })?;

    transaction.commit().await.map_err(|e| {
        postgis_error!("could not commit transaction: {}", e);
        PostgisError::FlightPath(FlightError::DBError)
    })?;

    postgis_info!("success.");
    Ok(())
}

/// Prepares a statement that checks zone intersections with the provided geometry
#[cfg(not(tarpaulin_include))]
// no_coverage: (R5) need psql backend to test
pub async fn get_flight_intersection_stmt(
    client: &Object,
) -> Result<tokio_postgres::Statement, PostgisError> {
    client
        .prepare_cached(&format!(
            r#"
            SELECT
                "flight_identifier",
                "aircraft_identifier",
                "geom",
                "time_start",
                "time_end",
                ST_3DLength(ST_Transform("geom", 4978)) as "distance",
                "distance_to_path"
            FROM {flights_table_name},
                ST_3DDistance(
                    ST_Transform("geom", 4978),
                    ST_Transform($1, 4978)
                ) as "distance_to_path"
            WHERE
                ("distance_to_path" < $2 OR "distance_to_path" IS NULL)
                AND ("time_start" <= $4 OR "time_start" IS NULL) -- easy checks first
                AND ("time_end" >= $3 OR "time_end" IS NULL)
                AND "simulated" = FALSE
        "#,
            flights_table_name = get_flights_table_name(),
        ))
        .await
        .map_err(|e| {
            postgis_error!("could not prepare cached statement: {}", e);
            PostgisError::FlightPath(FlightError::DBError)
        })
}

/// Splits intersecting flight paths into smaller segments to check for intersections
///  on a higher resolution
#[cfg(not(tarpaulin_include))]
// no_coverage: (R5) need psql backend to test
pub async fn intersection_check(
    client: &deadpool_postgres::Client,
    stmt: &tokio_postgres::Statement,
    allowable_distance: f64,
    segment_length: f32,
    a_segment: Segment,
    b_segment: Segment,
) -> Result<(), PostgisError> {
    postgis_debug!("entry.");
    let mut pairs: Vec<(Segment, Segment, f32)> = vec![(a_segment, b_segment, segment_length)];

    while let Some((a_segment, b_segment, segment_length)) = pairs.pop() {
        if (segment_length as f64) < allowable_distance {
            postgis_debug!("intersection < {allowable_distance} m found.");
            return Err(PostgisError::FlightPath(FlightError::Intersection));
        }

        postgis_debug!("subdividing segments with length: {}", segment_length);

        let a_segments = super::utils::segmentize(
            &a_segment.geom,
            a_segment.time_start,
            a_segment.time_end,
            segment_length,
        )
        .await
        .map_err(|e| {
            postgis_error!("could not segmentize path: {}", e);
            PostgisError::FlightPath(FlightError::DBError)
        })?;

        let b_segments = super::utils::segmentize(
            &b_segment.geom,
            b_segment.time_start,
            b_segment.time_end,
            segment_length,
        )
        .await
        .map_err(|e| {
            postgis_error!("could not segmentize path: {}", e);
            PostgisError::FlightPath(FlightError::DBError)
        })?;

        for a in &a_segments {
            for b in &b_segments {
                // look for time intersections
                if a.time_start > b.time_end || a.time_end < b.time_start {
                    continue;
                }

                let conflict: bool = client
                    .query_one(stmt, &[&a.geom, &b.geom, &allowable_distance])
                    .await
                    .map_err(|e| {
                        postgis_error!(
                            "could not query for existing flight paths intersection: {}",
                            e
                        );
                        PostgisError::FlightPath(FlightError::DBError)
                    })?
                    .try_get("conflict")
                    .map_err(|e| {
                        postgis_error!("could not get 'conflict' field: {}", e);

                        PostgisError::FlightPath(FlightError::DBError)
                    })?;

                if conflict {
                    postgis_debug!("found intersection, subdividing.");
                    pairs.push((a.clone(), b.clone(), segment_length / 2.0));
                }
            }
        }
    }

    Ok(())
}

#[cfg(not(tarpaulin_include))]
// no_coverage: (R5) need psql backend to test, no way to create a Row without querying it
//  from a postgres instance
fn process_row(
    row: tokio_postgres::Row,
    base: &Flight,
) -> Result<Flight, tokio_postgres::error::Error> {
    let mut flight = base.clone();
    let identifier: Option<String> = row.try_get("identifier")?;
    let session_id: Option<String> = row.try_get("session_id")?;
    let geom: PointZ = row.try_get("geom")?;
    let velocity_horizontal_ground_mps: f32 = row.try_get("velocity_horizontal_ground_mps")?;
    let velocity_vertical_mps: f32 = row.try_get("velocity_vertical_mps")?;
    let track_angle_degrees: f32 = row.try_get("track_angle_degrees")?;
    let last_position_update: DateTime<Utc> = row.try_get("last_position_update")?;
    let status: OperationalStatus = row.try_get("op_status")?;

    flight.session_id = session_id;
    flight.aircraft_id = identifier;
    flight.positions.push(TimePosition {
        position: Some(GrpcPointZ {
            latitude: geom.y,
            longitude: geom.x,
            altitude_meters: geom.z as f32,
        }),
        timestamp: Some(last_position_update.into()),
    });

    let state = AircraftState {
        timestamp: Some(last_position_update.into()),
        ground_speed_mps: velocity_horizontal_ground_mps,
        vertical_speed_mps: velocity_vertical_mps,
        track_angle_degrees,
        position: Some(GrpcPointZ {
            latitude: geom.y,
            longitude: geom.x,
            altitude_meters: geom.z as f32,
        }),
        status: status as i32,
    };

    flight.state = Some(state);

    Ok(flight)
}

/// Get flights and their aircraft that intersect with the provided geometry
///  and time range.
#[cfg(not(tarpaulin_include))]
// no_coverage: (R5) need psql backend to test
pub async fn get_flights(request: GetFlightsRequest) -> Result<Vec<Flight>, FlightError> {
    postgis_debug!("entry.");

    let time_start = request.time_start.ok_or_else(|| {
        postgis_error!("time_start is required.");
        FlightError::Time
    })?;

    let time_end = request.time_end.ok_or_else(|| {
        postgis_error!("time_end is required.");
        FlightError::Time
    })?;

    let time_start: DateTime<Utc> = time_start.into();
    let time_end: DateTime<Utc> = time_end.into();
    let linestring = LineStringT {
        points: vec![
            Point {
                x: request.window_min_x,
                y: request.window_min_y,
                srid: Some(DEFAULT_SRID),
            },
            Point {
                x: request.window_max_x,
                y: request.window_max_y,
                srid: Some(DEFAULT_SRID),
            },
        ],
        srid: Some(DEFAULT_SRID),
    };

    let client = crate::postgis::DEADPOOL_POSTGIS
        .get()
        .ok_or_else(|| {
            postgis_error!("could not get psql pool.");
            FlightError::Client
        })?
        .get()
        .await
        .map_err(|e| {
            postgis_error!("could not get client from psql connection pool: {}", e);
            FlightError::Client
        })?;

    let session_id_str = "flight_identifier";
    let aircraft_id_str = "aircraft_identifier";
    let aircraft_type_str = "aircraft_type";
    let simulated_str = "simulated";
    let stmt = client
        .prepare_cached(&format!(
            r#"
            SELECT 
                "flights"."flight_identifier" as "{session_id_str}",
                "aircraft"."identifier" as "{aircraft_id_str}",
                "aircraft"."aircraft_type" as "{aircraft_type_str}",
                "aircraft"."simulated" as "{simulated_str}"
            FROM {aircraft_table_name} as "aircraft"
            LEFT JOIN {flights_table_name} as "flights"
                ON (
                    "flights"."aircraft_identifier" = "aircraft"."identifier"
                    OR "flights"."flight_identifier" = "aircraft"."session_id"
                )
            WHERE 
                (
                    -- get grounded aircraft without a scheduled flight
                    ST_Intersects(ST_Envelope($1), "aircraft"."geom")
                    AND "aircraft"."last_position_update" >= $2
                    AND "aircraft"."last_position_update" <= $3
                ) OR (
                    -- flights that intersect this window
                    "flights"."geom" IS NOT NULL
                    AND ST_Intersects(ST_Envelope($1), "flights"."geom")
                    AND "flights"."time_end" >= $2
                    AND "flights"."time_start" <= $3
                );
            "#,
            flights_table_name = get_flights_table_name(),
            aircraft_table_name = super::aircraft::get_table_name(),
        ))
        .await
        .map_err(|e| {
            postgis_error!("could not prepare cached statement: {}", e);
            FlightError::DBError
        })?;

    let mut flights = client
        .query(&stmt, &[&linestring, &time_start, &time_end])
        .await
        .map_err(|e| {
            postgis_error!("could not execute transaction: {}", e);
            FlightError::DBError
        })?
        .iter()
        .map(|row| {
            let session_id: Option<String> = row.try_get(session_id_str)?;
            let aircraft_id: Option<String> = row.try_get(aircraft_id_str)?;
            let aircraft_type: AircraftType = row.try_get(aircraft_type_str)?;
            let simulated: bool = row.try_get(simulated_str)?;

            Ok(Flight {
                session_id,
                aircraft_id,
                simulated,
                positions: vec![],
                state: None,
                aircraft_type: aircraft_type as i32,
            })
        })
        .collect::<Result<Vec<Flight>, tokio_postgres::error::Error>>()
        .map_err(|e| {
            postgis_error!("could not get flight data: {}", e);
            FlightError::DBError
        })?;

    postgis_debug!("found {} flights.", flights.len());

    // TODO(R5): Change this to use Redis 60s telemetry storage to acquire
    //  telemetry information
    let stmt = client
        .prepare_cached(&format!(
            r#"SELECT
                    "identifier",
                    "session_id",
                    "geom",
                    "velocity_horizontal_ground_mps",
                    "velocity_vertical_mps",
                    "track_angle_degrees",
                    "last_position_update",
                    "op_status"
                FROM {table_name} 
                WHERE
                    "session_id" = $1 
                    OR "identifier" = $2 
                LIMIT 1;
        "#,
            table_name = super::aircraft::get_table_name(),
        ))
        .await
        .map_err(|e| {
            postgis_error!("could not prepare cached statement: {}", e);
            FlightError::DBError
        })?;

    let mut result: Vec<Flight> = vec![];
    for flight in &mut flights {
        let rows = client
            .query(&stmt, &[&flight.session_id, &flight.aircraft_id])
            .await;

        let rows = match rows {
            Ok(rows) => rows,
            Err(e) => {
                postgis_error!("could not execute transaction: {}", e);
                continue;
            }
        };

        let flight_it = rows.into_iter().filter_map(|row| {
            process_row(row, flight)
                .map_err(|e| {
                    postgis_error!("could not get position data for row: {e}");
                })
                .ok()
        });

        // 'extend' can take an iterator argument
        result.extend(flight_it);
    }

    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use lib_common::time::{Duration, Utc};

    #[tokio::test]
    async fn ut_client_failure() {
        lib_common::logger::get_log_handle().await;
        ut_info!("start");

        let item = UpdateFlightPathRequest {
            flight_identifier: Some("test".to_string()),
            aircraft_identifier: Some("test".to_string()),
            aircraft_type: AircraftType::Aeroplane as i32,
            simulated: false,
            timestamp_start: Some(Utc::now().into()),
            timestamp_end: Some((Utc::now() + Duration::try_hours(1).unwrap()).into()),
            path: vec![],
        };

        let result = update_flight_path(item).await.unwrap_err();
        assert_eq!(result, PostgisError::FlightPath(FlightError::DBError));

        ut_info!("success");
    }

    #[test]
    fn test_flight_error_display() {
        assert_eq!(
            FlightError::AircraftId.to_string(),
            "Invalid aircraft ID provided."
        );
        assert_eq!(
            FlightError::AircraftType.to_string(),
            "Invalid aircraft type provided."
        );
        assert_eq!(
            FlightError::Location.to_string(),
            "Invalid location provided."
        );
        assert_eq!(FlightError::Time.to_string(), "Invalid time provided.");
        assert_eq!(FlightError::Label.to_string(), "Invalid label provided.");
        assert_eq!(
            FlightError::Client.to_string(),
            "Could not get backend client."
        );
        assert_eq!(FlightError::DBError.to_string(), "Unknown backend error.");
        assert_eq!(
            FlightError::Segments.to_string(),
            "Could not segmentize path."
        );
        assert_eq!(
            FlightError::Intersection.to_string(),
            "Flight paths intersect."
        );
    }

    #[test]
    fn test_validate_flight_identifier() {
        let identifier = Some("test".to_string());

        // valid
        validate_flight_identifier(&identifier).unwrap();

        let identifier = None;
        let error = validate_flight_identifier(&identifier).unwrap_err();
        assert_eq!(error, PostgisError::FlightPath(FlightError::Label));

        let identifier = Some("".to_string());
        let error = validate_flight_identifier(&identifier).unwrap_err();
        assert_eq!(error, PostgisError::FlightPath(FlightError::Label));
    }
}
