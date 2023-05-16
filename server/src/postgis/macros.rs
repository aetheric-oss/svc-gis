//! log macro's for postgis logging

/// Writes a debug! message to the app::postgis logger
#[macro_export]
macro_rules! postgis_debug {
    ($($arg:tt)+) => {
        log::debug!(target: "app::postgis", $($arg)+)
    };
}

/// Writes an info! message to the app::postgis logger
#[macro_export]
macro_rules! postgis_info {
    ($($arg:tt)+) => {
        log::info!(target: "app::postgis", $($arg)+)
    };
}

/// Writes an warn! message to the app::postgis logger
#[macro_export]
macro_rules! postgis_warn {
    ($($arg:tt)+) => {
        log::warn!(target: "app::postgis", $($arg)+)
    };
}

/// Writes an error! message to the app::postgis logger
#[macro_export]
macro_rules! postgis_error {
    ($($arg:tt)+) => {
        log::error!(target: "app::postgis", $($arg)+)
    };
}
