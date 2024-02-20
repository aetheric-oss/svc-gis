#![doc = include_str!("./README.md")]

use strum::IntoEnumIterator;

#[macro_use]
pub mod macros;
// pub mod nearest;
pub mod aircraft;
pub mod best_path;
pub mod flight;
pub mod pool;
pub mod utils;
pub mod vertiport;
pub mod waypoint;
pub mod zone;

pub use once_cell::sync::OnceCell;

/// Global pool for PostgreSQL connections
pub static DEADPOOL_POSTGIS: OnceCell<deadpool_postgres::Pool> = OnceCell::new();

/// PostgreSQL schema for all tables
pub const PSQL_SCHEMA: &str = "arrow";

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
    FlightPath(flight::FlightPathError),
}

impl std::error::Error for PostgisError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        None
    }
}

impl std::fmt::Display for PostgisError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
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

impl std::fmt::Display for PsqlError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PsqlError::Client => write!(f, "Client Error"),
            PsqlError::Connection => write!(f, "Connection Error"),
            PsqlError::Execute => write!(f, "Error on execution"),
            PsqlError::Rollback => write!(f, "Error on rollback"),
            PsqlError::Commit => write!(f, "Error on commit"),
        }
    }
}

impl std::error::Error for PsqlError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        None
    }
}

/// Executes a transaction with multiple statements on the provided pool
///  with rollback if any of the statements fail to execute.
pub async fn psql_transaction(statements: Vec<String>) -> Result<(), PostgisError> {
    let Some(pool) = DEADPOOL_POSTGIS.get() else {
        postgis_error!("(psql_transaction) could not get psql pool.");
        return Err(PostgisError::Psql(PsqlError::Client));
    };

    let mut client = pool.get().await.map_err(|e| {
        postgis_error!(
            "(psql_transaction) could not get client from psql connection pool: {}",
            e
        );
        PostgisError::Psql(PsqlError::Client)
    })?;

    let transaction = client.transaction().await.map_err(|e| {
        postgis_error!("(psql_transaction) could not create transaction: {}", e);
        PostgisError::Psql(PsqlError::Connection)
    })?;

    for stmt in statements.into_iter() {
        if let Err(e) = transaction.execute(&stmt, &[]).await {
            postgis_error!("(psql_transaction) Failed to execute statement '{stmt}': {e}");

            transaction.rollback().await.map_err(|e| {
                postgis_error!("(psql_transaction) Failed to rollback transaction: {}", e);
                PostgisError::Psql(PsqlError::Rollback)
            })?;

            return Err(PostgisError::Psql(PsqlError::Execute));
        }
    }

    transaction.commit().await.map_err(|e| {
        postgis_error!("(psql_transaction) Failed to commit transaction: {}", e);
        PostgisError::Psql(PsqlError::Commit)
    })?;

    Ok(())
}

/// Generates a PostgreSQL enum declaration from a Rust enum
pub fn psql_enum_declaration<T>(enum_name: &str) -> String
where
    T: IntoEnumIterator + std::fmt::Display,
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
    postgis_info!("(psql_enum_declaration) {}.", declaration);

    declaration
}

/// Initializes the PostgreSQL database with the required tables and enums
pub async fn psql_init() -> Result<(), Box<dyn std::error::Error>> {
    zone::psql_init().await?;
    vertiport::psql_init().await?;
    aircraft::psql_init().await?;
    waypoint::psql_init().await?;
    flight::psql_init().await?;

    Ok(())
}

/// Performs maintenance tasks (removing old records, etc.) on the PostgreSQL database
pub async fn psql_maintenance() -> Result<(), Box<dyn std::error::Error>> {
    flight::psql_maintenance().await?;

    Ok(())
}
