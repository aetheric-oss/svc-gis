//! This module contains functions for updating aircraft in the PostGIS database.

use super::{psql_transaction, PostgisError, DEFAULT_SRID, PSQL_SCHEMA};

use crate::cache::{Consumer, Processor};
use crate::postgis::utils::StringError;
use chrono::{DateTime, Utc};
use postgis::ewkb::PointZ;
use tonic::async_trait;

use crate::types::{AircraftId, AircraftPosition, AircraftType, AircraftVelocity};

/// Allowed characters in a identifier
pub const IDENTIFIER_REGEX: &str = r"^[\-0-9A-Za-z_\.]{1,255}$";

/// Possible errors with aircraft requests
#[derive(Debug, Copy, Clone, PartialEq)]
pub enum AircraftError {
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
            AircraftError::AircraftId => write!(f, "Invalid aircraft ID provided."),
            AircraftError::Location => write!(f, "Invalid location provided."),
            AircraftError::Time => write!(f, "Invalid time provided."),
            AircraftError::Label => write!(f, "Invalid label provided."),
            AircraftError::Client => write!(f, "Could not get backend client."),
            AircraftError::DBError => write!(f, "Unknown backend error."),
        }
    }
}

/// Gets the name of this module's table
fn get_table_name() -> &'static str {
    static FULL_NAME: &str = const_format::formatcp!(r#""{PSQL_SCHEMA}"."aircraft""#,);
    FULL_NAME
}

/// Verifies that a identifier is valid
pub fn check_identifier(identifier: &str) -> Result<(), StringError> {
    super::utils::check_string(identifier, IDENTIFIER_REGEX)
}

/// Initializes the PostGIS database for aircraft.
pub async fn psql_init() -> Result<(), PostgisError> {
    // Create Aircraft Table
    let enum_name = "aircrafttype";
    let statements = vec![
        super::psql_enum_declaration::<AircraftType>(enum_name),
        format!(
            r#"CREATE TABLE IF NOT EXISTS {table_name} (
                "identifier" VARCHAR(20) UNIQUE PRIMARY KEY NOT NULL,
                "aircraft_type" {enum_name} NOT NULL DEFAULT '{enum_default}',
                "velocity_horizontal_ground_mps" FLOAT(4),
                "velocity_horizontal_air_mps" FLOAT(4),
                "velocity_vertical_mps" FLOAT(4),
                "track_angle_degrees" FLOAT(4),
                "geom" GEOMETRY(POINTZ, {DEFAULT_SRID}),
                "last_identifier_update" TIMESTAMPTZ,
                "last_position_update" TIMESTAMPTZ,
                "last_velocity_update" TIMESTAMPTZ
            );"#,
            table_name = get_table_name(),
            enum_default = AircraftType::Undeclared.to_string()
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

        update_aircraft_id(items).await.map_err(|_| ())
    }
}

#[async_trait]
impl Processor<AircraftPosition> for Consumer {
    async fn process(&mut self, items: Vec<AircraftPosition>) -> Result<(), ()> {
        if items.is_empty() {
            return Ok(());
        }

        update_aircraft_position(items).await.map_err(|_| ())
    }
}

#[async_trait]
impl Processor<AircraftVelocity> for Consumer {
    async fn process(&mut self, items: Vec<AircraftVelocity>) -> Result<(), ()> {
        if items.is_empty() {
            return Ok(());
        }

        update_aircraft_velocity(items).await.map_err(|_| ())
    }
}

/// Validates the provided aircraft identification.
fn validate_identification(item: &AircraftId, now: &DateTime<Utc>) -> Result<(), PostgisError> {
    if let Err(e) = check_identifier(&item.identifier) {
        postgis_error!(
            "(validate_identification) invalid identifier {}: {}",
            item.identifier,
            e
        );

        return Err(PostgisError::Aircraft(AircraftError::Label));
    }

    if item.timestamp_network > *now {
        postgis_error!(
            "(validate_identification) could not validate timestamp_network (in future): {}",
            item.timestamp_network
        );

        return Err(PostgisError::Aircraft(AircraftError::Time));
    }

    Ok(())
}

/// Pulls queued aircraft id messages from Redis Queue
/// Updates aircraft in the PostGIS database.
/// Confirms with Redis Queue that item was processed.
pub async fn update_aircraft_id(aircraft: Vec<AircraftId>) -> Result<(), PostgisError> {
    postgis_debug!("(update_aircraft_id) entry.");
    if aircraft.is_empty() {
        return Ok(());
    }

    let Some(pool) = crate::postgis::DEADPOOL_POSTGIS.get() else {
        postgis_error!("(update_aircraft_id) could not get psql pool.");
        return Err(PostgisError::Aircraft(AircraftError::Client));
    };

    let mut client = pool.get().await.map_err(|e| {
        postgis_error!(
            "(update_aircraft_id) could not get client from psql connection pool: {}",
            e
        );

        PostgisError::Aircraft(AircraftError::Client)
    })?;

    let transaction = client.transaction().await.map_err(|e| {
        postgis_error!("(update_aircraft_id) could not create transaction: {}", e);

        PostgisError::Aircraft(AircraftError::DBError)
    })?;

    let stmt = transaction
        .prepare_cached(&format!(
            r#"
        INSERT INTO {table_name} (
            "identifier",
            "aircraft_type",
            "last_identifier_update"
        )
        VALUES ($1, $2, $3)
        ON CONFLICT ("identifier") DO UPDATE
            SET "aircraft_type" = EXCLUDED."aircraft_type",
                "last_identifier_update" = EXCLUDED."last_identifier_update";
        "#,
            table_name = get_table_name()
        ))
        .await
        .map_err(|e| {
            postgis_error!(
                "(update_aircraft_id) could not prepare cached statement: {}",
                e
            );
            PostgisError::Aircraft(AircraftError::DBError)
        })?;

    let now = Utc::now();
    for craft in &aircraft {
        if let Err(e) = validate_identification(craft, &now) {
            postgis_error!(
                "(update_aircraft_id) could not validate id for aircraft {}: {:?}",
                craft.identifier,
                e
            );

            continue;
        }

        transaction
            .execute(
                &stmt,
                &[
                    &craft.identifier,
                    &craft.aircraft_type,
                    &craft.timestamp_network,
                ],
            )
            .await
            .map_err(|e| {
                postgis_error!("(update_aircraft_id) could not execute transaction: {}", e);
                PostgisError::Aircraft(AircraftError::DBError)
            })?;
    }

    match transaction.commit().await {
        Ok(_) => {
            postgis_debug!("(update_aircraft_id) success.");
            Ok(())
        }
        Err(e) => {
            postgis_error!("(update_aircraft_id) could not commit transaction: {}", e);
            Err(PostgisError::Aircraft(AircraftError::DBError))
        }
    }
}

/// Validates the provided aircraft position.
fn validate_position(item: &AircraftPosition, now: &DateTime<Utc>) -> Result<(), PostgisError> {
    if item.position.latitude < -90.0 || item.position.latitude > 90.0 {
        postgis_error!(
            "(validate_position) could not validate latitude: {}",
            item.position.latitude
        );
        return Err(PostgisError::Aircraft(AircraftError::Location));
    }

    if item.position.longitude < -180.0 || item.position.longitude > 180.0 {
        postgis_error!(
            "(validate_position) could not validate longitude: {}",
            item.position.longitude
        );

        return Err(PostgisError::Aircraft(AircraftError::Location));
    }

    if let Err(e) = check_identifier(&item.identifier) {
        postgis_error!(
            "(validate_position) invalid identifier {}: {}",
            item.identifier,
            e
        );

        return Err(PostgisError::Aircraft(AircraftError::Label));
    }

    if item.timestamp_network > *now {
        postgis_error!(
            "(validate_position) could not validate timestamp_network (in future): {}",
            item.timestamp_network
        );

        return Err(PostgisError::Aircraft(AircraftError::Time));
    }

    Ok(())
}

/// Updates aircraft position in the PostGIS database.
pub async fn update_aircraft_position(aircraft: Vec<AircraftPosition>) -> Result<(), PostgisError> {
    postgis_debug!("(update_aircraft_position) entry.");
    if aircraft.is_empty() {
        return Ok(());
    }

    let Some(pool) = crate::postgis::DEADPOOL_POSTGIS.get() else {
        postgis_error!("(update_aircraft_position) could not get psql pool.");
        return Err(PostgisError::Aircraft(AircraftError::Client));
    };

    let mut client = pool.get().await.map_err(|e| {
        postgis_error!(
            "(update_aircraft_position) could not get client from psql connection pool: {}",
            e
        );
        PostgisError::Aircraft(AircraftError::Client)
    })?;

    let transaction = client.transaction().await.map_err(|e| {
        postgis_error!(
            "(update_aircraft_position) could not create transaction: {}",
            e
        );
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
            postgis_error!(
                "(update_aircraft_position) could not prepare cached statement: {}",
                e
            );
            PostgisError::Aircraft(AircraftError::DBError)
        })?;

    let now = Utc::now();
    for craft in &aircraft {
        if let Err(e) = validate_position(craft, &now) {
            postgis_error!(
                "(update_aircraft_position) could not validate position for aircraft {}: {:?}",
                craft.identifier,
                e
            );

            continue;
        }

        let Ok(geom) = PointZ::try_from(craft.position) else {
            postgis_error!(
                "(update_aircraft_position) could not convert position to PointZ for aircraft {}: {:?}",
                craft.identifier,
                craft.position
            );

            continue;
        };

        transaction
            .execute(&stmt, &[&craft.identifier, &geom, &craft.timestamp_network])
            .await
            .map_err(|e| {
                postgis_error!(
                    "(update_aircraft_position) could not execute transaction: {}",
                    e
                );
                PostgisError::Aircraft(AircraftError::DBError)
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
            Err(PostgisError::Aircraft(AircraftError::DBError))
        }
    }
}

/// Validates the provided aircraft velocity
fn validate_velocity(item: &AircraftVelocity, now: &DateTime<Utc>) -> Result<(), PostgisError> {
    if let Err(e) = check_identifier(&item.identifier) {
        postgis_error!(
            "(validate_velocity) invalid identifier {}: {}",
            item.identifier,
            e
        );

        return Err(PostgisError::Aircraft(AircraftError::Label));
    }

    if item.timestamp_network > *now {
        postgis_error!(
            "(validate_velocity) could not validate timestamp_network (in future): {}",
            item.timestamp_network
        );

        return Err(PostgisError::Aircraft(AircraftError::Time));
    }

    Ok(())
}

/// Updates aircraft velocity in the PostGIS database.
pub async fn update_aircraft_velocity(aircraft: Vec<AircraftVelocity>) -> Result<(), PostgisError> {
    postgis_debug!("(update_aircraft_velocity) entry.");
    if aircraft.is_empty() {
        return Ok(());
    }

    let Some(pool) = crate::postgis::DEADPOOL_POSTGIS.get() else {
        postgis_error!("(update_aircraft_velocity) could not get psql pool.");
        return Err(PostgisError::Aircraft(AircraftError::Client));
    };

    let mut client = pool.get().await.map_err(|e| {
        postgis_error!(
            "(update_aircraft_velocity) could not get client from psql connection pool: {}",
            e
        );
        PostgisError::Aircraft(AircraftError::Client)
    })?;

    let transaction = client.transaction().await.map_err(|e| {
        postgis_error!(
            "(update_aircraft_velocity) could not create transaction: {}",
            e
        );
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
        ) ON CONFLICT (identifier) DO UPDATE
            SET "velocity_horizontal_ground_mps" = EXCLUDED."velocity_horizontal_ground_mps",
                "velocity_vertical_mps" = EXCLUDED."velocity_vertical_mps",
                "track_angle_degrees" = EXCLUDED."track_angle_degrees",
                "last_velocity_update" = EXCLUDED."last_velocity_update";"#,
            table_name = get_table_name()
        ))
        .await
        .map_err(|e| {
            postgis_error!(
                "(update_aircraft_velocity) could not prepare cached statement: {}",
                e
            );
            PostgisError::Aircraft(AircraftError::DBError)
        })?;

    let now = Utc::now();
    for craft in &aircraft {
        if let Err(e) = validate_velocity(craft, &now) {
            postgis_error!(
                "(update_aircraft_velocity) could not validate velocity for aircraft {}: {:?}",
                craft.identifier,
                e
            );

            continue;
        }

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
                postgis_error!(
                    "(update_aircraft_velocity) could not execute transaction: {}",
                    e
                );
                PostgisError::Aircraft(AircraftError::DBError)
            })?;
    }

    match transaction.commit().await {
        Ok(_) => {
            postgis_debug!("(update_aircraft_velocity) success.");
            Ok(())
        }
        Err(e) => {
            postgis_error!(
                "(update_aircraft_velocity) could not commit transaction: {}",
                e
            );
            Err(PostgisError::Aircraft(AircraftError::DBError))
        }
    }
}

/// Gets the geometry of an aircraft given its identifier.
pub async fn get_aircraft_pointz(identifier: &str) -> Result<PointZ, PostgisError> {
    let stmt = format!(
        r#"SELECT "geom" FROM {table_name} WHERE "identifier" = $1;"#,
        table_name = get_table_name()
    );

    let Some(pool) = crate::postgis::DEADPOOL_POSTGIS.get() else {
        postgis_error!("(get_aircraft_pointz) could not get psql pool.");
        return Err(PostgisError::Aircraft(AircraftError::Client));
    };

    let client = pool.get().await.map_err(|e| {
        postgis_error!(
            "(get_aircraft_pointz) could not get client from psql connection pool: {}",
            e
        );
        PostgisError::Aircraft(AircraftError::Client)
    })?;

    client
        .query_one(&stmt, &[&identifier])
        .await
        .map_err(|e| {
            postgis_error!("(get_aircraft_pointz) could not prepare cached statement: {}", e);
            PostgisError::Aircraft(AircraftError::DBError)
        })?
        .try_get::<_, PointZ>(0)
        .map_err(|e| {
            postgis_error!("(get_aircraft_pointz) zero or more than one records found for aircraft '{identifier}': {}", e);
            PostgisError::Aircraft(AircraftError::DBError)
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::Position;
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
        assert_eq!(result, PostgisError::Aircraft(AircraftError::Client));

        ut_info!("(ut_client_failure) success");
    }

    #[tokio::test]
    async fn ut_aircraft_to_gis_invalid_label() {
        crate::get_log_handle().await;
        ut_info!("(ut_aircraft_position_to_gis_invalid_label) start");

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
                identifier: label.to_string(),
                timestamp_network: Utc::now(),
                aircraft_type: AircraftType::Rotorcraft,
                timestamp_asset: None,
            };

            let result = validate_position(&position, &Utc::now()).unwrap_err();
            assert_eq!(result, PostgisError::Aircraft(AircraftError::Label));

            let result = validate_velocity(&velocity, &Utc::now()).unwrap_err();
            assert_eq!(result, PostgisError::Aircraft(AircraftError::Label));

            let result = validate_identification(&id, &Utc::now()).unwrap_err();
            assert_eq!(result, PostgisError::Aircraft(AircraftError::Label));
        }

        ut_info!("(ut_aircraft_position_to_gis_invalid_label) success");
    }

    #[tokio::test]
    async fn ut_aircraft_position_to_gis_invalid_location() {
        crate::get_log_handle().await;
        ut_info!("(ut_aircraft_position_to_gis_invalid_location) start");

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

            let result = validate_position(&aircraft, &Utc::now()).unwrap_err();
            assert_eq!(result, PostgisError::Aircraft(AircraftError::Location));
        }

        ut_info!("(ut_aircraft_position_to_gis_invalid_location) success");
    }

    #[tokio::test]
    async fn ut_aircraft_position_to_gis_invalid_time() {
        crate::get_log_handle().await;
        ut_info!("(ut_aircraft_position_to_gis_invalid_time) start");

        let timestamp_network = Utc::now() + Duration::days(1);
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
            identifier: "Aircraft".to_string(),
            aircraft_type: AircraftType::Rotorcraft,
            timestamp_asset: None,
        };

        let result = validate_position(&position, &Utc::now()).unwrap_err();
        assert_eq!(result, PostgisError::Aircraft(AircraftError::Time));

        let result = validate_velocity(&velocity, &Utc::now()).unwrap_err();
        assert_eq!(result, PostgisError::Aircraft(AircraftError::Time));

        let result = validate_identification(&id, &Utc::now()).unwrap_err();
        assert_eq!(result, PostgisError::Aircraft(AircraftError::Time));

        ut_info!("(ut_aircraft_position_to_gis_invalid_time) success");
    }
}
