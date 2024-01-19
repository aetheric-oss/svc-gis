#![doc = include_str!("./README.md")]

use strum::IntoEnumIterator;

#[macro_use]
pub mod macros;
pub mod aircraft;
pub mod best_path;
// pub mod nearest;
pub mod pool;
pub mod utils;
pub mod vertiport;
pub mod waypoint;
pub mod zone;

/// Error type for postgis actions
#[derive(Debug, Copy, Clone)]
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
pub async fn psql_transaction(
    statements: Vec<String>,
    pool: &deadpool_postgres::Pool,
) -> Result<(), PsqlError> {
    let mut client = pool.get().await.map_err(|e| {
        postgis_error!(
            "(psql_init) could not get client from psql connection pool: {}",
            e
        );
        PsqlError::Client
    })?;

    let transaction = client.transaction().await.map_err(|e| {
        postgis_error!("(psql_init) could not create transaction: {}", e);
        PsqlError::Connection
    })?;

    for stmt in statements.into_iter() {
        if let Err(e) = transaction.execute(&stmt, &[]).await {
            postgis_error!("(psql_init) Failed to execute statement '{stmt}': {e}");

            transaction.rollback().await.map_err(|e| {
                postgis_error!("(psql_init) Failed to rollback transaction: {}", e);
                PsqlError::Rollback
            })?;

            return Err(PsqlError::Execute);
        }
    }

    transaction.commit().await.map_err(|e| {
        postgis_error!("(psql_init) Failed to commit transaction: {}", e);
        PsqlError::Commit
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

    let declaration = format!("CREATE TYPE {enum_name} as ENUM ({fields});");
    postgis_info!("(psql_enum_declaration) {}.", declaration);

    declaration
}

/// Initializes the PostgreSQL database with the required tables and enums
pub async fn psql_init(pool: &deadpool_postgres::Pool) -> Result<(), Box<dyn std::error::Error>> {
    zone::psql_init(pool).await?;
    vertiport::psql_init(pool).await?;
    aircraft::psql_init(pool).await?;
    waypoint::psql_init(pool).await?;

    Ok(())
}
