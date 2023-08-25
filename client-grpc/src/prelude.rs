//! Re-export of used objects

pub use super::client as gis;
pub use super::service::Client as GisServiceClient;
pub use gis::GisClient;

pub use lib_common::grpc::Client;
pub use postgres_types::FromSql;
pub use prost_wkt_types::Timestamp;
