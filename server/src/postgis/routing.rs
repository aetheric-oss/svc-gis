//! This module contains functions for routing between nodes.

use chrono::{DateTime, Utc};
use uuid::Uuid;

// TODO(R4): Include altitude, lanes, corridors
const ALTITUDE_HARDCODE: f64 = 1000.0;

/// A segment of a path between two nodes at a given altitude
#[derive(Debug, Clone, Copy)]
pub struct PathSegment {
    /// The index of the segment in the path
    pub index: i32,

    /// The UUID of the node at the start of the segment
    pub node_uuid_start: Uuid,

    /// The UUID of the node at the end of the segment
    pub node_uuid_end: Uuid,

    /// The distance between the two nodes
    pub distance_meters: f64,

    /// The altitude of the segment
    pub altitude_meters: f64,
}

/// The purpose of this initial search is to verify that a flight between two
///  vertiports is physically possible.
///
/// A flight is physically impossible if the two vertiports cannot be
///  connected by a series of lines such that the aircraft never runs out
///  of charge.
///
/// No-Fly zones can extend flights, isolate aircraft, or disable vertiports entirely.
pub async fn best_path(
    node_uuid_start: Uuid,
    node_uuid_end: Uuid,
    time_start: DateTime<Utc>,
    time_end: DateTime<Utc>,
    pool: deadpool_postgres::Pool,
) -> Result<Vec<PathSegment>, ()> {
    let cmd_str = format!(
        "SELECT * FROM best_path(
            '{node_uuid_start}'::UUID,
            '{node_uuid_end}'::UUID,
            '{time_start}'::timestamptz,
            '{time_end}'::timestamptz
        );"
    );

    let client = match pool.get().await {
        Ok(client) => client,
        Err(e) => {
            println!("(get_paths) could not get client from pool.");
            println!("(get_paths) error: {:?}", e);
            return Err(());
        }
    };

    let rows = match client.query(&cmd_str, &[]).await {
        Ok(results) => results,
        Err(e) => {
            println!("(get_paths) could not request routes: {}", e);
            return Err(());
        }
    };

    let mut results: Vec<PathSegment> = vec![];
    for r in &rows {
        let start_id: Uuid = r.get(1);
        let end_id: Uuid = r.get(2);

        results.push(PathSegment {
            index: r.get(0),
            node_uuid_start: start_id,
            node_uuid_end: end_id,
            distance_meters: r.get(3),
            altitude_meters: ALTITUDE_HARDCODE, // TODO(R4): Corridors
        });
    }

    Ok(results)
}
