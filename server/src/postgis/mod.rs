#![doc = include_str!("./README.md")]

use strum::IntoEnumIterator;

#[macro_use]
pub mod macros;
pub mod aircraft;
pub mod best_path;
pub mod flight;
pub mod pool;
pub mod utils;
pub mod vertiport;
pub mod waypoint;
pub mod zone;

pub use once_cell::sync::OnceCell;
use std::fmt::{self, Display, Formatter};

/// Global pool for PostgreSQL connections
pub static DEADPOOL_POSTGIS: OnceCell<deadpool_postgres::Pool> = OnceCell::new();

/// PostgreSQL schema for all tables
pub const PSQL_SCHEMA: &str = "arrow";

/// Default Spatial Reference Identifier
/// WGS84 with Z axis: <https://spatialreference.org/ref/epsg/4326/>
pub const DEFAULT_SRID: i32 = 4326;

/// Error type for postgis actions
#[derive(Debug, Copy, Clone, PartialEq)]
pub enum PostgisError {
    /// PostgreSQL Error
    Psql(PsqlError),

    /// Vertiport Error
    Vertiport(vertiport::VertiportError),

    /// Aircraft Error
    Aircraft(aircraft::AircraftError),

    /// Waypoint Error
    Waypoint(waypoint::WaypointError),

    /// Zone Error
    Zone(zone::ZoneError),

    /// BestPath Error
    BestPath(best_path::PathError),

    /// FlightPath Error
    FlightPath(flight::FlightError),
}

impl std::error::Error for PostgisError {}

impl Display for PostgisError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            PostgisError::Psql(e) => write!(f, "PostgreSQL Error: {}", e),
            PostgisError::Vertiport(e) => write!(f, "Vertiport Error: {}", e),
            PostgisError::Aircraft(e) => write!(f, "Aircraft Error: {}", e),
            PostgisError::Waypoint(e) => write!(f, "Waypoint Error: {}", e),
            PostgisError::Zone(e) => write!(f, "Zone Error: {}", e),
            PostgisError::BestPath(e) => write!(f, "BestPath Error: {}", e),
            PostgisError::FlightPath(e) => write!(f, "FlightPath Error: {}", e),
        }
    }
}

/// Error type for postgis actions
#[derive(Debug, Copy, Clone, PartialEq)]
pub enum PsqlError {
    /// Client Error
    Client,

    /// Connection Error
    Connection,

    /// Error on statement execution
    Execute,

    /// Error on rollback
    Rollback,

    /// Error on commit
    Commit,
}

impl Display for PsqlError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            PsqlError::Client => write!(f, "Client Error"),
            PsqlError::Connection => write!(f, "Connection Error"),
            PsqlError::Execute => write!(f, "Error on execution"),
            PsqlError::Rollback => write!(f, "Error on rollback"),
            PsqlError::Commit => write!(f, "Error on commit"),
        }
    }
}

impl std::error::Error for PsqlError {}

/// Executes a transaction with multiple statements on the provided pool
///  with rollback if any of the statements fail to execute.
#[cfg(not(tarpaulin_include))]
// no_coverage: (Rnever) need running postgresql instance
pub async fn psql_transaction(statements: Vec<String>) -> Result<(), PostgisError> {
    let pool = DEADPOOL_POSTGIS.get().ok_or_else(|| {
        postgis_error!("could not get psql pool.");
        PostgisError::Psql(PsqlError::Connection)
    })?;

    let mut client = pool.get().await.map_err(|e| {
        postgis_error!("could not get client from psql connection pool: {}", e);
        PostgisError::Psql(PsqlError::Client)
    })?;

    let transaction = client.transaction().await.map_err(|e| {
        postgis_error!("could not create transaction: {}", e);
        PostgisError::Psql(PsqlError::Client)
    })?;

    for stmt in statements.into_iter() {
        let Err(e) = transaction.execute(&stmt, &[]).await else {
            continue;
        };

        postgis_error!("Failed to execute statement '{stmt}': {e}");

        transaction.rollback().await.map_err(|e| {
            postgis_error!("Failed to rollback transaction: {}", e);
            PostgisError::Psql(PsqlError::Rollback)
        })?;

        return Err(PostgisError::Psql(PsqlError::Execute));
    }

    transaction.commit().await.map_err(|e| {
        postgis_error!("Failed to commit transaction: {}", e);
        PostgisError::Psql(PsqlError::Commit)
    })?;

    Ok(())
}

/// Generates a PostgreSQL enum declaration from a Rust enum
pub fn psql_enum_declaration<T>(enum_name: &str) -> String
where
    T: IntoEnumIterator + Display,
{
    let fields = T::iter()
        .map(|field| format!("'{}'", field))
        .collect::<Vec<String>>()
        .join(", ");

    let declaration = format!(
        "DO $$
    BEGIN
        IF NOT EXISTS (SELECT 1 FROM pg_type WHERE typname = '{enum_name}') THEN
            CREATE TYPE {enum_name} as ENUM ({fields});
        END IF;
    END $$;"
    );

    postgis_info!("{}.", declaration);

    declaration
}

/// Initializes the PostgreSQL database with the required tables and enums
#[cfg(not(tarpaulin_include))]
// no_coverage: (Rnever) need running postgresql instance, not unit testable
pub async fn psql_init() -> Result<(), Box<dyn std::error::Error>> {
    zone::psql_init().await?;
    vertiport::psql_init().await?;
    aircraft::psql_init().await?;
    waypoint::psql_init().await?;
    flight::psql_init().await?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_postgis_error_display() {
        let error = PostgisError::Psql(PsqlError::Client);
        assert_eq!(error.to_string(), "PostgreSQL Error: Client Error");

        let error = PostgisError::Vertiport(vertiport::VertiportError::Identifier);
        assert_eq!(
            error.to_string(),
            format!("Vertiport Error: {}", vertiport::VertiportError::Identifier)
        );

        let error = PostgisError::Aircraft(aircraft::AircraftError::Identifier);
        assert_eq!(
            error.to_string(),
            format!("Aircraft Error: {}", aircraft::AircraftError::Identifier)
        );

        let error = PostgisError::Waypoint(waypoint::WaypointError::Identifier);
        assert_eq!(
            error.to_string(),
            format!("Waypoint Error: {}", waypoint::WaypointError::Identifier)
        );

        let error = PostgisError::Zone(zone::ZoneError::Identifier);
        assert_eq!(
            error.to_string(),
            format!("Zone Error: {}", zone::ZoneError::Identifier)
        );

        let error = PostgisError::BestPath(best_path::PathError::Internal);
        assert_eq!(
            error.to_string(),
            format!("BestPath Error: {}", best_path::PathError::Internal)
        );

        let error = PostgisError::FlightPath(flight::FlightError::Time);
        assert_eq!(
            error.to_string(),
            format!("FlightPath Error: {}", flight::FlightError::Time)
        );
    }

    #[test]
    fn test_psql_error_display() {
        let error = PsqlError::Client;
        assert_eq!(error.to_string(), "Client Error");

        let error = PsqlError::Connection;
        assert_eq!(error.to_string(), "Connection Error");

        let error = PsqlError::Execute;
        assert_eq!(error.to_string(), "Error on execution");

        let error = PsqlError::Rollback;
        assert_eq!(error.to_string(), "Error on rollback");

        let error = PsqlError::Commit;
        assert_eq!(error.to_string(), "Error on commit");
    }

    #[test]
    fn test_psql_enum_declaration() {
        #[derive(strum::EnumIter, Debug)]
        enum TestEnum {
            A,
            B,
            C,
        }

        impl Display for TestEnum {
            fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
                write!(f, "{:?}", self)
            }
        }

        let re = regex::Regex::new(r"\s{2,}").unwrap();
        let enum_name = "test_enum";
        let declaration = psql_enum_declaration::<TestEnum>(enum_name);
        let expected = format!(
            "DO $$
        BEGIN
            IF NOT EXISTS (SELECT 1 FROM pg_type WHERE typname = '{enum_name}') THEN
                CREATE TYPE {enum_name} as ENUM ('A', 'B', 'C');
            END IF;
        END $$;"
        );

        // different indenting
        let expected = re.replace_all(&expected, " ");
        let declaration = re.replace_all(&declaration, " ");
        assert_eq!(declaration, expected);
    }
}
