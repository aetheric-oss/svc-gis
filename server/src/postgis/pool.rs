//! Secure connections to the PostGIS database
//!

use deadpool_postgres::{ManagerConfig, Pool, RecyclingMethod, Runtime};
use native_tls::{Certificate, Identity, TlsConnector};
use postgres_native_tls::MakeTlsConnector;
// use tokio_postgres::tls::MakeTlsConnect;

use crate::config::Config;
use std::fmt::{self, Display, Formatter};
use std::fs;

/// Errors that can occur when creating a connection pool
#[derive(Debug, Copy, Clone, PartialEq)]
pub enum PoolError {
    /// Unable to load authority certificate
    AuthorityCertificate,

    /// Unable to create pem
    AuthorityPem,

    /// Unable to load client certificate
    ClientCertificate,

    /// Unable to create client key
    ClientKey,

    /// Unable to create identity
    Identity,

    /// Unable to build connector
    Builder,

    /// Unable to create pool connection
    Connection,
}

impl Display for PoolError {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        match self {
            PoolError::AuthorityCertificate => write!(f, "unable to load authority certificate"),
            PoolError::AuthorityPem => write!(f, "unable to create pem"),
            PoolError::ClientCertificate => write!(f, "unable to load client certificate"),
            PoolError::ClientKey => write!(f, "unable to create client key"),
            PoolError::Identity => write!(f, "unable to create identity"),
            PoolError::Builder => write!(f, "unable to build connector"),
            PoolError::Connection => write!(f, "unable to create pool connection"),
        }
    }
}

/// Creates a connection to the PostGIS database using SSL certificates
pub fn create_pool(mut config: Config) -> Result<Pool, PoolError> {
    config.pg.manager = Some(ManagerConfig {
        recycling_method: RecyclingMethod::Fast,
    });

    let client_cert = config.db_client_cert;
    let client_key = config.db_client_key;

    let root_cert_file = fs::read(config.db_ca_cert.clone()).map_err(|e| {
        postgis_error!(
            "unable to read db_ca_cert file [{}]: {e}",
            config.db_ca_cert
        );

        PoolError::AuthorityCertificate
    })?;

    let root_cert = Certificate::from_pem(&root_cert_file).map_err(|e| {
        postgis_error!(
            "unable to load Certificate from pem file [{}]: {}",
            config.db_ca_cert,
            e
        );

        PoolError::AuthorityPem
    })?;

    let client_cert_file = fs::read(client_cert).map_err(|e| {
        postgis_error!(
            "(create_pool) unable to read client certificate db_client_cert file: {}",
            e
        );
        PoolError::ClientCertificate
    })?;

    let client_key_file = fs::read(client_key).map_err(|e| {
        postgis_error!(
            "(create_pool) unable to read client key db_client_key file: {}",
            e
        );
        PoolError::ClientKey
    })?;

    let identity = Identity::from_pkcs8(&client_cert_file, &client_key_file).map_err(|e| {
        postgis_error!(
            "(create_pool) unable to create identity from specified cert and key: {}",
            e
        );

        PoolError::Identity
    })?;

    let connector = TlsConnector::builder()
        .add_root_certificate(root_cert)
        .identity(identity)
        .build()
        .map_err(|e| {
            postgis_error!(
                "(create_pool) unable to connect build connector custom ca and client certs: {}",
                e
            );

            PoolError::Builder
        })?;

    let connector = MakeTlsConnector::new(connector);
    config
        .pg
        .create_pool(Some(Runtime::Tokio1), connector)
        .map_err(|e| {
            postgis_error!("(create_pool) unable to create pool connection: {}", e);

            PoolError::Connection
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;

    // fn fake_pem() -> String {
    //     format!(
    //         "-----BEGIN CERTIFICATE-----\n{}\n-----END CERTIFICATE-----\n",
    //         "ABCDEFGHIJKLMNOPQRSTUVWXYZ1234567890"
    //     )
    // }

    #[test]
    fn test_create_pool_invalid_db_ca_cert() {
        let mut config = Config::new();

        // file doesn't exist
        config.db_ca_cert = "/".to_string(); // invalid
        let error = create_pool(config.clone()).unwrap_err();
        assert_eq!(error, PoolError::AuthorityCertificate);

        // file exists, invalid pem
        // let fname = "invalid_ca.pem";
        // let mut config = Config::new();
        // config.db_ca_cert = fname.to_string();
        // fs::write(&fname, fake_pem()).unwrap();
        // let error = create_pool(config.clone()).unwrap_err();
        // assert_eq!(error, PoolError::AuthorityPem);
    }

    #[test]
    fn test_pool_error_display() {
        assert_eq!(
            PoolError::AuthorityCertificate.to_string(),
            "unable to load authority certificate"
        );
        assert_eq!(PoolError::AuthorityPem.to_string(), "unable to create pem");
        assert_eq!(
            PoolError::ClientCertificate.to_string(),
            "unable to load client certificate"
        );
        assert_eq!(
            PoolError::ClientKey.to_string(),
            "unable to create client key"
        );
        assert_eq!(PoolError::Identity.to_string(), "unable to create identity");
        assert_eq!(PoolError::Builder.to_string(), "unable to build connector");
        assert_eq!(
            PoolError::Connection.to_string(),
            "unable to create pool connection"
        );
    }
}
