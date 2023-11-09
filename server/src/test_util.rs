use deadpool_postgres::{ManagerConfig, Pool, RecyclingMethod, Runtime};
/// test utilities. Provides functions to inject mock data.
use lib_common::log_macros;
use tokio::sync::OnceCell;
use tokio_postgres::NoTls;

log_macros!("ut", "test");

/// Create global variable to access our database pool
pub(crate) static DB_POOL: OnceCell<Pool> = OnceCell::const_new();
pub(crate) async fn get_psql_pool() -> &'static Pool {
    DB_POOL
        .get_or_init(|| async move {
            let mut cfg = deadpool_postgres::Config::default();
            cfg.dbname = Some("deadpool".to_string());
            cfg.manager = Some(ManagerConfig {
                recycling_method: RecyclingMethod::Fast,
            });
            cfg.create_pool(Some(Runtime::Tokio1), NoTls).unwrap()
        })
        .await
}
