//! Secure connections to the PostGIS database
//!

use deadpool_postgres::{ManagerConfig, Pool, RecyclingMethod, Runtime};
use native_tls::{Certificate, Identity, TlsConnector};
use postgres_native_tls::MakeTlsConnector;

use crate::config::Config;
use std::fs;

/// Creates a connection to the PostGIS database using SSL certificates
pub fn create_pool(mut config: Config) -> Pool {
    config.pg.manager = Some(ManagerConfig {
        recycling_method: RecyclingMethod::Fast,
    });

    let client_cert = config.db_client_cert;
    let client_key = config.db_client_key;

    let root_cert_file = fs::read(config.db_ca_cert.clone()).unwrap_or_else(|e| {
        panic!(
            "Unable to read db_ca_cert file [{}]: {}",
            config.db_ca_cert, e
        )
    });

    let root_cert = Certificate::from_pem(&root_cert_file).unwrap_or_else(|e| {
        panic!(
            "Unable to load Certificate from pem file [{}]: {}",
            config.db_ca_cert, e
        )
    });

    let client_cert_file = fs::read(client_cert).unwrap_or_else(|e| {
        panic!(
            "Unable to read client certificate db_client_cert file: {}",
            e
        );
    });

    let client_key_file = fs::read(client_key).unwrap_or_else(|e| {
        panic!("Unable to read client key db_client_key file: {}", e);
    });

    let builder = TlsConnector::builder()
        .add_root_certificate(root_cert)
        .identity(
            Identity::from_pkcs8(&client_cert_file, &client_key_file).unwrap_or_else(|e| {
                panic!(
                    "Unable to create identity from specified cert and key: {}",
                    e
                );
            }),
        )
        .build()
        .unwrap_or_else(|e| {
            panic!(
                "Unable to connect build connector custom ca and client certs: {}",
                e
            )
        });

    let connector = MakeTlsConnector::new(builder);

    let result = config.pg.create_pool(Some(Runtime::Tokio1), connector);
    match result {
        Ok(pool) => pool,
        Err(e) => {
            panic!("Error creating pool: {}", e);
        }
    }
}
