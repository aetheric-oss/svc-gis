#![doc = include_str!("./README.md")]

#[macro_use]
pub mod macros;
pub mod node;
pub mod nofly;
pub mod pool;

async fn execute_psql_cmd(cmd_str: String, pool: deadpool_postgres::Pool) -> Result<(), ()> {
    println!("{}", &cmd_str);

    // Get PSQL Client
    let client = match pool.get().await {
        Ok(client) => client,
        Err(e) => {
            postgis_error!("(execute_psql_cmd) Error getting client: {}", e);
            return Err(());
        }
    };

    // Execute command
    match client.execute(&cmd_str, &[]).await {
        Ok(_) => Ok(()),
        Err(e) => {
            postgis_error!("(execute_psql_cmd) Error executing command: {}", e);
            Err(())
        }
    }
}
