//! Re-export of used objects

pub use super::client as gis;
pub use super::service::Client as GisServiceClient;
pub use gis::GisClient;

/// Types used with svc-gis Redis queues
pub mod types {
    include!("../../common/types.rs");
}

pub use lib_common::grpc::Client;
pub use lib_common::time::Timestamp;
pub use postgres_types::FromSql;
pub use types::*;
