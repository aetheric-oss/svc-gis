//! This module contains functions for updating aircraft in the PostGIS database.

use super::{psql_transaction, PostgisError, DEFAULT_SRID, PSQL_SCHEMA};

use crate::cache::{Consumer, Processor};
use lib_common::time::{DateTime, Utc};
use postgis::ewkb::PointZ;
use std::fmt::{self, Display, Formatter};
use tonic::async_trait;

use crate::types::{
    AircraftId, AircraftPosition, AircraftType, AircraftVelocity, OperationalStatus,
};

/// Allowed characters in a identifier
pub const IDENTIFIER_REGEX: &str = r"^[\-0-9A-Za-z_\.]{1,255}$";

/// Possible errors with aircraft requests
#[derive(Debug, Copy, Clone, PartialEq)]
pub enum AircraftError {
    /// Invalid Location
    Location,

    /// Invalid Time Provided
    Time,

    /// Invalid Identifier
    Identifier,

    /// No Aircraft
    NoAircraft,

    /// Could not get client
    Client,

    /// DBError error
    DBError,
}

impl Display for AircraftError {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        match self {
            AircraftError::Location => write!(f, "Invalid location provided."),
            AircraftError::Time => write!(f, "Invalid time provided."),
            AircraftError::Identifier => write!(f, "Invalid identifier(s) provided."),
            AircraftError::NoAircraft => write!(f, "No aircraft provided."),
            AircraftError::Client => write!(f, "Could not get backend client."),
            AircraftError::DBError => write!(f, "Unknown backend error."),
        }
    }
}

/// Gets the name of this module's table
pub(super) fn get_table_name() -> &'static str {
    static FULL_NAME: &str = const_format::formatcp!(r#""{PSQL_SCHEMA}"."aircraft""#,);
    FULL_NAME
}

/// Verifies that a identifier is valid
pub fn check_identifier(identifier: &str) -> Result<(), PostgisError> {
    super::utils::check_string(identifier, IDENTIFIER_REGEX).map_err(|e| {
        postgis_error!("invalid identifier: {e}");
        PostgisError::Aircraft(AircraftError::Identifier)
    })
}

/// Initializes the PostGIS database for aircraft.
#[cfg(not(tarpaulin_include))]
// no_coverage: (R5) needs psql backend to test
pub async fn psql_init() -> Result<(), PostgisError> {
    // Create Aircraft Table
    let type_enum_name = "aircrafttype";
    let status_enum_name = "opstatus";
    let statements = vec![
        super::psql_enum_declaration::<AircraftType>(type_enum_name),
        super::psql_enum_declaration::<OperationalStatus>(status_enum_name),
        format!(
            r#"CREATE TABLE IF NOT EXISTS {table_name} (
                "identifier" VARCHAR(20) UNIQUE PRIMARY KEY,
                "session_id" VARCHAR(20) UNIQUE,
                "aircraft_type" {type_enum_name} NOT NULL DEFAULT '{type_enum_default}',
                "velocity_horizontal_ground_mps" FLOAT(4),
                "velocity_horizontal_air_mps" FLOAT(4),
                "velocity_vertical_mps" FLOAT(4),
                "track_angle_degrees" FLOAT(4),
                "geom" GEOMETRY(POINTZ, {DEFAULT_SRID}),
                "last_identifier_update" TIMESTAMPTZ,
                "last_position_update" TIMESTAMPTZ,
                "last_velocity_update" TIMESTAMPTZ,
                "simulated" BOOLEAN DEFAULT FALSE,
                "op_status" {status_enum_name} NOT NULL DEFAULT '{status_enum_default}'
            );"#,
            table_name = get_table_name(),
            type_enum_default = AircraftType::Undeclared.to_string(),
            status_enum_default = OperationalStatus::Undeclared.to_string()
        ),
    ];

    psql_transaction(statements).await
}

#[async_trait]
impl Processor<AircraftId> for Consumer {
    async fn process(&mut self, items: Vec<AircraftId>) -> Result<(), ()> {
        if items.is_empty() {
            return Ok(());
        }

        #[cfg(not(tarpaulin_include))]
        // no_coverage: (R5) needs psql backend to test
        update_aircraft_id(items).await.map_err(|_| ())
    }
}

#[async_trait]
impl Processor<AircraftPosition> for Consumer {
    async fn process(&mut self, items: Vec<AircraftPosition>) -> Result<(), ()> {
        if items.is_empty() {
            return Ok(());
        }

        #[cfg(not(tarpaulin_include))]
        // no_coverage: (R5) needs psql backend to test
        update_aircraft_position(items).await.map_err(|_| ())
    }
}

#[async_trait]
impl Processor<AircraftVelocity> for Consumer {
    async fn process(&mut self, items: Vec<AircraftVelocity>) -> Result<(), ()> {
        if items.is_empty() {
            return Ok(());
        }

        #[cfg(not(tarpaulin_include))]
        // no_coverage: (R5) needs psql backend to test
        update_aircraft_velocity(items).await.map_err(|_| ())
    }
}

/// Validates the provided aircraft identification.
fn validate_identification(
    caa_identifier: &Option<String>,
    session_id: &Option<String>,
) -> Result<(), PostgisError> {
    if caa_identifier.is_none() && session_id.is_none() {
        postgis_error!(
            "aircraft ID must have at least one of: [CAA-assigned aircraft ID, session ID]"
        );

        return Err(PostgisError::Aircraft(AircraftError::Identifier));
    }

    if let Some(identifier) = caa_identifier {
        check_identifier(identifier)?;
    }

    if let Some(identifier) = session_id {
        super::flight::check_flight_identifier(identifier).map_err(|e| {
            postgis_error!("invalid session_id {:?}: {e}", identifier);
            PostgisError::Aircraft(AircraftError::Identifier)
        })?;
    }

    Ok(())
}

/// Validates the provided aircraft identification.
fn validate_id_message(item: &AircraftId, now: &DateTime<Utc>) -> Result<(), PostgisError> {
    validate_identification(&item.identifier, &item.session_id)?;

    if item.timestamp_network > *now {
        postgis_error!(
            "could not validate timestamp_network (in future): {}",
            item.timestamp_network
        );

        return Err(PostgisError::Aircraft(AircraftError::Time));
    }

    Ok(())
}

/// Pulls queued aircraft id messages from Redis Queue
/// Updates aircraft in the PostGIS database.
/// Confirms with Redis Queue that item was processed.
#[cfg(not(tarpaulin_include))]
// no_coverage: (R5) needs psql backend to test
pub async fn update_aircraft_id(aircraft: Vec<AircraftId>) -> Result<(), PostgisError> {
    postgis_debug!("entry.");

    let now = Utc::now();
    let aircraft: Vec<AircraftId> = aircraft
        .into_iter()
        .filter(|item| validate_id_message(item, &now).is_ok())
        .collect();

    if aircraft.is_empty() {
        return Err(PostgisError::Aircraft(AircraftError::NoAircraft));
    }

    let pool = crate::postgis::DEADPOOL_POSTGIS.get().ok_or_else(|| {
        postgis_error!("could not get psql pool.");
        PostgisError::Aircraft(AircraftError::Client)
    })?;

    let mut client = pool.get().await.map_err(|e| {
        postgis_error!("could not get client from psql connection pool: {}", e);

        PostgisError::Aircraft(AircraftError::Client)
    })?;

    let transaction = client.transaction().await.map_err(|e| {
        postgis_error!("could not create transaction: {}", e);

        PostgisError::Aircraft(AircraftError::DBError)
    })?;

    let stmt = transaction
        .prepare_cached(&format!(
            r#"
        INSERT INTO {table_name} (
            "identifier",
            "session_id",
            "aircraft_type",
            "last_identifier_update"
        )
        VALUES ($1, $2, $3, $4)
        ON CONFLICT ("identifier") DO UPDATE
            SET "session_id" = EXCLUDED."session_id",
                "aircraft_type" = EXCLUDED."aircraft_type",
                "last_identifier_update" = EXCLUDED."last_identifier_update";
        "#,
            table_name = get_table_name()
        ))
        .await
        .map_err(|e| {
            postgis_error!("could not prepare cached statement: {}", e);
            PostgisError::Aircraft(AircraftError::DBError)
        })?;

    for craft in &aircraft {
        transaction
            .execute(
                &stmt,
                &[
                    &craft.identifier,
                    &craft.session_id,
                    &craft.aircraft_type,
                    &craft.timestamp_network,
                ],
            )
            .await
            .map_err(|e| {
                postgis_error!("could not execute transaction: {}", e);
                PostgisError::Aircraft(AircraftError::DBError)
            })?;
    }

    transaction.commit().await.map_err(|e| {
        postgis_error!("could not commit transaction: {}", e);
        PostgisError::Aircraft(AircraftError::DBError)
    })?;

    postgis_debug!("success.");
    Ok(())
}

/// Validates the provided aircraft position.
fn validate_position_message(
    item: &AircraftPosition,
    now: &DateTime<Utc>,
) -> Result<(), PostgisError> {
    if item.position.latitude < -90.0 || item.position.latitude > 90.0 {
        postgis_error!("could not validate latitude: {}", item.position.latitude);
        return Err(PostgisError::Aircraft(AircraftError::Location));
    }

    if item.position.longitude < -180.0 || item.position.longitude > 180.0 {
        postgis_error!("could not validate longitude: {}", item.position.longitude);

        return Err(PostgisError::Aircraft(AircraftError::Location));
    }

    if item.timestamp_network > *now {
        postgis_error!(
            "could not validate timestamp_network (in future): {}",
            item.timestamp_network
        );

        return Err(PostgisError::Aircraft(AircraftError::Time));
    }

    check_identifier(&item.identifier)?;

    Ok(())
}

/// Updates aircraft position in the PostGIS database.
#[cfg(not(tarpaulin_include))]
// no_coverage: (R5) needs psql backend to test
pub async fn update_aircraft_position(aircraft: Vec<AircraftPosition>) -> Result<(), PostgisError> {
    postgis_debug!("entry.");

    let now = Utc::now();
    let aircraft: Vec<AircraftPosition> = aircraft
        .into_iter()
        .filter(|item| validate_position_message(item, &now).is_ok())
        .collect();

    if aircraft.is_empty() {
        return Err(PostgisError::Aircraft(AircraftError::NoAircraft));
    }

    let pool = crate::postgis::DEADPOOL_POSTGIS.get().ok_or_else(|| {
        postgis_error!("could not get psql pool.");
        PostgisError::Aircraft(AircraftError::Client)
    })?;

    let mut client = pool.get().await.map_err(|e| {
        postgis_error!("could not get client from psql connection pool: {}", e);
        PostgisError::Aircraft(AircraftError::Client)
    })?;

    let transaction = client.transaction().await.map_err(|e| {
        postgis_error!("could not create transaction: {}", e);
        PostgisError::Aircraft(AircraftError::DBError)
    })?;

    let stmt = transaction
        .prepare_cached(&format!(
            r#"
        INSERT INTO {table_name} (
            "identifier",
            "geom",
            "last_position_update"
        )
        VALUES ($1, $2, $3)
        ON CONFLICT ("identifier") DO UPDATE
            SET "geom" = EXCLUDED."geom",
                "last_position_update" = EXCLUDED."last_position_update";
        "#,
            table_name = get_table_name()
        ))
        .await
        .map_err(|e| {
            postgis_error!("could not prepare cached statement: {}", e);
            PostgisError::Aircraft(AircraftError::DBError)
        })?;

    for craft in &aircraft {
        let geom = PointZ::from(craft.position);

        transaction
            .execute(&stmt, &[&craft.identifier, &geom, &craft.timestamp_network])
            .await
            .map_err(|e| {
                postgis_error!("could not execute transaction: {}", e);
                PostgisError::Aircraft(AircraftError::DBError)
            })?;
    }

    transaction.commit().await.map_err(|e| {
        postgis_error!("could not commit transaction: {}", e);
        PostgisError::Aircraft(AircraftError::DBError)
    })?;

    postgis_debug!("success.");
    Ok(())
}

/// Validates the provided aircraft velocity
fn validate_velocity_message(
    item: &AircraftVelocity,
    now: &DateTime<Utc>,
) -> Result<(), PostgisError> {
    check_identifier(&item.identifier)?;

    if item.timestamp_network > *now {
        postgis_error!(
            "could not validate timestamp_network (in future): {}",
            item.timestamp_network
        );

        return Err(PostgisError::Aircraft(AircraftError::Time));
    }

    Ok(())
}

/// Updates aircraft velocity in the PostGIS database.
#[cfg(not(tarpaulin_include))]
// no_coverage: (R5) needs psql backend to test
pub async fn update_aircraft_velocity(aircraft: Vec<AircraftVelocity>) -> Result<(), PostgisError> {
    postgis_debug!("entry.");

    let now = Utc::now();
    let aircraft: Vec<AircraftVelocity> = aircraft
        .into_iter()
        .filter(|item| validate_velocity_message(item, &now).is_ok())
        .collect();

    if aircraft.is_empty() {
        return Err(PostgisError::Aircraft(AircraftError::NoAircraft));
    }

    let pool = crate::postgis::DEADPOOL_POSTGIS.get().ok_or_else(|| {
        postgis_error!("could not get psql pool.");
        PostgisError::Aircraft(AircraftError::Client)
    })?;

    let mut client = pool.get().await.map_err(|e| {
        postgis_error!("could not get client from psql connection pool: {}", e);
        PostgisError::Aircraft(AircraftError::Client)
    })?;

    let transaction = client.transaction().await.map_err(|e| {
        postgis_error!("could not create transaction: {}", e);
        PostgisError::Aircraft(AircraftError::DBError)
    })?;

    let stmt = transaction
        .prepare_cached(&format!(
            r#"
        INSERT INTO {table_name} (
            "identifier",
            "velocity_horizontal_ground_mps",
            "velocity_vertical_mps",
            "track_angle_degrees",
            "last_velocity_update"
        ) VALUES (
            $1, $2, $3, $4, $5
        ) ON CONFLICT ("identifier") DO UPDATE
            SET "velocity_horizontal_ground_mps" = EXCLUDED."velocity_horizontal_ground_mps",
                "velocity_vertical_mps" = EXCLUDED."velocity_vertical_mps",
                "track_angle_degrees" = EXCLUDED."track_angle_degrees",
                "last_velocity_update" = EXCLUDED."last_velocity_update";"#,
            table_name = get_table_name()
        ))
        .await
        .map_err(|e| {
            postgis_error!("could not prepare cached statement: {}", e);
            PostgisError::Aircraft(AircraftError::DBError)
        })?;

    for craft in &aircraft {
        transaction
            .execute(
                &stmt,
                &[
                    &craft.identifier,
                    &craft.velocity_horizontal_ground_mps,
                    &craft.velocity_vertical_mps,
                    &craft.track_angle_degrees,
                    &craft.timestamp_network,
                ],
            )
            .await
            .map_err(|e| {
                postgis_error!("could not execute transaction: {}", e);
                PostgisError::Aircraft(AircraftError::DBError)
            })?;
    }

    transaction.commit().await.map_err(|e| {
        postgis_error!("could not commit transaction: {}", e);
        PostgisError::Aircraft(AircraftError::DBError)
    })?;

    postgis_debug!("success.");
    Ok(())
}

/// Gets the geometry of an aircraft given its identifier.
#[cfg(not(tarpaulin_include))]
// no_coverage: (R5) needs psql backend to test
pub async fn get_aircraft_pointz(identifier: &str) -> Result<PointZ, PostgisError> {
    let stmt = format!(
        r#"SELECT "geom" FROM {table_name} WHERE "identifier" = $1;"#,
        table_name = get_table_name()
    );

    let pool = crate::postgis::DEADPOOL_POSTGIS.get().ok_or_else(|| {
        postgis_error!("could not get psql pool.");
        PostgisError::Aircraft(AircraftError::Client)
    })?;

    let client = pool.get().await.map_err(|e| {
        postgis_error!("could not get client from psql connection pool: {}", e);
        PostgisError::Aircraft(AircraftError::Client)
    })?;

    client
        .query_one(&stmt, &[&identifier])
        .await
        .map_err(|e| {
            postgis_error!("could not prepare cached statement: {}", e);
            PostgisError::Aircraft(AircraftError::DBError)
        })?
        .try_get::<_, PointZ>("geom")
        .map_err(|e| {
            postgis_error!(
                "zero or more than one records found for aircraft '{identifier}': {}",
                e
            );
            PostgisError::Aircraft(AircraftError::DBError)
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::Position;
    use lib_common::time::Duration;

    #[tokio::test]
    async fn ut_client_failure() {
        lib_common::logger::get_log_handle().await;
        ut_info!("start");

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
        assert_eq!(result, PostgisError::Aircraft(AircraftError::Client));

        ut_info!("success");
    }

    #[tokio::test]
    async fn ut_aircraft_to_gis_invalid_label() {
        lib_common::logger::get_log_handle().await;
        ut_info!("start");

        for label in &[
            "NULL",
            "Aircraft;",
            "'Aircraft'",
            "Aircraft \'",
            &"X".repeat(1000),
        ] {
            let position = AircraftPosition {
                identifier: label.to_string(),
                position: Position {
                    latitude: 0.0,
                    longitude: 0.0,
                    altitude_meters: 100.0,
                },
                timestamp_network: Utc::now(),
                timestamp_asset: None,
            };

            let velocity = AircraftVelocity {
                identifier: label.to_string(),
                timestamp_network: Utc::now(),
                velocity_horizontal_ground_mps: 0.0,
                velocity_horizontal_air_mps: None,
                velocity_vertical_mps: 0.0,
                track_angle_degrees: 0.0,
                timestamp_asset: None,
            };

            let id = AircraftId {
                identifier: Some(label.to_string()),
                session_id: None,
                timestamp_network: Utc::now(),
                aircraft_type: AircraftType::Rotorcraft,
                timestamp_asset: None,
            };

            let result = validate_position_message(&position, &Utc::now()).unwrap_err();
            assert_eq!(result, PostgisError::Aircraft(AircraftError::Identifier));

            let result = validate_velocity_message(&velocity, &Utc::now()).unwrap_err();
            assert_eq!(result, PostgisError::Aircraft(AircraftError::Identifier));

            let result = validate_id_message(&id, &Utc::now()).unwrap_err();
            assert_eq!(result, PostgisError::Aircraft(AircraftError::Identifier));
        }

        ut_info!("success");
    }

    #[tokio::test]
    async fn ut_aircraft_id_no_identifier() {
        lib_common::logger::get_log_handle().await;
        ut_info!("start");

        let id = AircraftId {
            identifier: None,
            session_id: None,
            timestamp_network: Utc::now(),
            aircraft_type: AircraftType::Rotorcraft,
            timestamp_asset: None,
        };

        let result = validate_id_message(&id, &Utc::now()).unwrap_err();
        assert_eq!(result, PostgisError::Aircraft(AircraftError::Identifier));

        ut_info!("success");
    }

    #[tokio::test]
    async fn ut_aircraft_position_to_gis_invalid_location() {
        lib_common::logger::get_log_handle().await;
        ut_info!("start");

        let coords = vec![(-90.1, 0.0), (90.1, 0.0), (0.0, -180.1), (0.0, 180.1)];
        for coord in coords {
            let aircraft = AircraftPosition {
                position: Position {
                    latitude: coord.0,
                    longitude: coord.1,
                    altitude_meters: 100.0,
                },
                identifier: "Aircraft".to_string(),
                timestamp_network: Utc::now(),
                timestamp_asset: None,
            };

            let result = validate_position_message(&aircraft, &Utc::now()).unwrap_err();
            assert_eq!(result, PostgisError::Aircraft(AircraftError::Location));
        }

        ut_info!("success");
    }

    #[tokio::test]
    async fn ut_aircraft_position_to_gis_invalid_time() {
        lib_common::logger::get_log_handle().await;
        ut_info!("start");

        let timestamp_network = Utc::now() + Duration::try_days(1).unwrap();
        let position = AircraftPosition {
            timestamp_network,
            position: Position {
                latitude: 0.0,
                longitude: 0.0,
                altitude_meters: 0.0,
            },
            identifier: "Aircraft".to_string(),
            timestamp_asset: None,
        };

        let velocity = AircraftVelocity {
            timestamp_network,
            identifier: "Aircraft".to_string(),
            velocity_horizontal_ground_mps: 0.0,
            velocity_horizontal_air_mps: None,
            velocity_vertical_mps: 0.0,
            track_angle_degrees: 0.0,
            timestamp_asset: None,
        };

        let id = AircraftId {
            timestamp_network,
            identifier: Some("Aircraft".to_string()),
            session_id: None,
            aircraft_type: AircraftType::Rotorcraft,
            timestamp_asset: None,
        };

        let result = validate_position_message(&position, &Utc::now()).unwrap_err();
        assert_eq!(result, PostgisError::Aircraft(AircraftError::Time));

        let result = validate_velocity_message(&velocity, &Utc::now()).unwrap_err();
        assert_eq!(result, PostgisError::Aircraft(AircraftError::Time));

        let result = validate_id_message(&id, &Utc::now()).unwrap_err();
        assert_eq!(result, PostgisError::Aircraft(AircraftError::Time));

        ut_info!("success");
    }

    #[test]
    fn test_aircraft_error_display() {
        assert_eq!(
            format!("{}", AircraftError::Location),
            "Invalid location provided."
        );
        assert_eq!(format!("{}", AircraftError::Time), "Invalid time provided.");
        assert_eq!(
            format!("{}", AircraftError::Identifier),
            "Invalid identifier(s) provided."
        );
        assert_eq!(
            format!("{}", AircraftError::Client),
            "Could not get backend client."
        );
        assert_eq!(
            format!("{}", AircraftError::DBError),
            "Unknown backend error."
        );
        assert_eq!(
            format!("{}", AircraftError::NoAircraft),
            "No aircraft provided."
        );
    }

    #[test]
    fn test_validate_identification() {
        validate_identification(
            &Some("Aircraft".to_string()),
            &Some("AETH12345".to_string()),
        )
        .unwrap();

        let error = validate_identification(&None, &None).unwrap_err();
        assert_eq!(error, PostgisError::Aircraft(AircraftError::Identifier));

        let error = validate_identification(&Some("///".to_string()), &None).unwrap_err();
        assert_eq!(error, PostgisError::Aircraft(AircraftError::Identifier));

        let error = validate_identification(&None, &Some("///".to_string())).unwrap_err();
        assert_eq!(error, PostgisError::Aircraft(AircraftError::Identifier));
    }

    #[test]
    fn test_get_table_name() {
        assert_eq!(get_table_name(), r#""arrow"."aircraft""#);
    }

    #[tokio::test]
    async fn test_update_aircraft_id() {
        let aircraft = vec![];
        let error = update_aircraft_id(aircraft).await.unwrap_err();
        assert_eq!(error, PostgisError::Aircraft(AircraftError::NoAircraft));
    }

    #[tokio::test]
    async fn test_update_aircraft_position() {
        let aircraft = vec![];
        let error = update_aircraft_position(aircraft).await.unwrap_err();
        assert_eq!(error, PostgisError::Aircraft(AircraftError::NoAircraft));
    }

    #[tokio::test]
    async fn test_update_aircraft_velocity() {
        let aircraft = vec![];
        let error = update_aircraft_velocity(aircraft).await.unwrap_err();
        assert_eq!(error, PostgisError::Aircraft(AircraftError::NoAircraft));
    }
}
