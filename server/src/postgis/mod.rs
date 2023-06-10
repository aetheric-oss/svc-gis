#![doc = include_str!("./README.md")]

#[macro_use]
pub mod macros;
pub mod aircraft;
pub mod nofly;
pub mod pool;
pub mod routing;
pub mod utils;
pub mod vertiport;
pub mod waypoint;

use postgres_types::FromSql;

/// Types of nodes returned by the routing algorithm
#[derive(Debug, Copy, Clone, FromSql)]
#[postgres(name = "nodetype")]
pub enum NodeType {
    /// Vertiport Node
    #[postgres(name = "vertiport")]
    Vertiport,

    /// Waypoint Node
    #[postgres(name = "waypoint")]
    Waypoint,

    /// Aircraft Node
    #[postgres(name = "aircraft")]
    Aircraft,
}

impl From<NodeType> for crate::grpc::server::NodeType {
    fn from(node_type: NodeType) -> Self {
        match node_type {
            NodeType::Vertiport => crate::grpc::server::NodeType::Vertiport,
            NodeType::Waypoint => crate::grpc::server::NodeType::Waypoint,
            NodeType::Aircraft => crate::grpc::server::NodeType::Aircraft,
        }
    }
}

#[cfg(not(tarpaulin_include))]
async fn execute_transaction(
    commands: Vec<String>,
    pool: deadpool_postgres::Pool,
) -> Result<(), ()> {
    if commands.is_empty() {
        return Ok(());
    }

    // Get PSQL Client
    let mut client = match pool.get().await {
        Ok(client) => client,
        Err(e) => {
            postgis_error!("(execute_transaction) Error getting client: {}", e);
            return Err(());
        }
    };

    // Transaction Builder
    let transaction = match client.build_transaction().start().await {
        Ok(transaction) => transaction,
        Err(e) => {
            postgis_error!("(execute_transaction) Error starting transaction: {}", e);
            return Err(());
        }
    };

    // Execute command
    for cmd_str in commands {
        if let Err(e) = transaction.execute(&cmd_str, &[]).await {
            postgis_error!("(execute_transaction) Error executing command: {}", e);
            // Rollback will automatically occur when the connection is dropped
            return Err(());
        }
    }

    if let Err(e) = transaction.commit().await {
        postgis_error!("(execute_transaction) Error committing transaction: {}", e);
        return Err(());
    }

    Ok(())
}
