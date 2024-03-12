//! Common functions for PostGIS operations

use super::DEFAULT_SRID;
use super::{PostgisError, PsqlError};
use crate::grpc::server::grpc_server::{Coordinates, PointZ as GrpcPointZ};
use crate::types::Position;
use chrono::{DateTime, Duration, Utc};
use deadpool_postgres::tokio_postgres::{types::ToSql, Row};
use geo::algorithm::haversine_distance::HaversineDistance;
use geo::point;
use postgis::ewkb::{LineStringT, LineStringZ, Point, PointZ, PolygonZ};
use regex;

/// A polygon must have at least three vertices (a triangle)
/// A closed polygon has the first and last vertex equal
/// Therefore, four vertices needed to indicate a closed triangular region
pub const MIN_NUM_POLYGON_VERTICES: usize = 4;

/// Errors converting vertices to a PostGIS Polygon
#[derive(Debug, Copy, Clone, PartialEq)]
pub enum PolygonError {
    /// Not enough vertices
    VertexCount,

    /// First and last vertices not equal
    OpenPolygon,

    /// A vertex does not fit within the valid range of latitude and longitude
    OutOfBounds,
}

impl std::fmt::Display for PolygonError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            PolygonError::VertexCount => write!(f, "Invalid number of vertices provided."),
            PolygonError::OpenPolygon => write!(
                f,
                "The first and last vertices do not match (open polygon)."
            ),
            PolygonError::OutOfBounds => write!(f, "One or more vertices are out of bounds."),
        }
    }
}

/// Errors converting a vertex to a PostGIS point
#[derive(Debug, Copy, Clone, PartialEq)]
pub enum PointError {
    /// A vertex does not fit within the valid range of latitude and longitude
    OutOfBounds,
}

impl std::fmt::Display for PointError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            PointError::OutOfBounds => write!(f, "One or more vertices are out of bounds."),
        }
    }
}

#[derive(Debug, Copy, Clone, PartialEq)]
/// Errors validating a string
pub enum StringError {
    /// Regex is invalid
    Regex,

    /// Provided string contains invalid keywords
    ContainsForbidden,

    /// Provided string doesn't match regex
    Mismatch,
}

impl std::fmt::Display for StringError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            StringError::Regex => write!(f, "Regex is invalid."),
            StringError::Mismatch => write!(f, "String does not match regex."),
            StringError::ContainsForbidden => write!(f, "String contains 'null'."),
        }
    }
}

/// Check if a provided string argument is valid
pub fn check_string(string: &str, regex: &str) -> Result<(), StringError> {
    let Ok(re) = regex::Regex::new(regex) else {
        return Err(StringError::Regex);
    };

    if string.to_lowercase().contains("null") {
        return Err(StringError::ContainsForbidden);
    }

    if !re.is_match(string) {
        return Err(StringError::Mismatch);
    }

    Ok(())
}

/// Approximate the distance between these two points
pub fn distance_meters(a: &PointZ, b: &PointZ) -> f32 {
    let p1 = point!(x: a.x, y: a.y);
    let p2 = point!(x: b.x, y: b.y);

    let distance_meters = p1.haversine_distance(&p2);

    // the Z coordinate is already in meters
    (distance_meters.powf(2.) + (a.z - b.z).powf(2.)).sqrt() as f32
}

/// Validate a PointZ
pub fn validate_pointz(point: &PointZ) -> Result<(), PolygonError> {
    if point.x < -180.0 || point.x > 180.0 || point.y < -90.0 || point.y > 90.0 {
        return Err(PolygonError::OutOfBounds);
    }

    Ok(())
}

impl TryFrom<Position> for PointZ {
    type Error = ();

    fn try_from(position: Position) -> Result<Self, Self::Error> {
        Ok(PointZ::new(
            position.longitude,
            position.latitude,
            position.altitude_meters,
            Some(DEFAULT_SRID),
        ))
    }
}

impl TryFrom<GrpcPointZ> for PointZ {
    type Error = ();

    fn try_from(position: GrpcPointZ) -> Result<Self, Self::Error> {
        Ok(PointZ::new(
            position.longitude,
            position.latitude,
            position.altitude_meters as f64,
            Some(DEFAULT_SRID),
        ))
    }
}

/// Generate a PostGIS Polygon from a list of vertices
/// The first and last vertices must be equal
/// The polygon must have at least [`MIN_NUM_POLYGON_VERTICES`] vertices
/// Each vertex must be within the valid range of latitude and longitude
pub fn polygon_from_vertices_z(
    vertices: &[Coordinates],
    altitude_meters: f32,
) -> Result<PolygonZ, PolygonError> {
    let size = vertices.len();

    // Check that the zone has at least N vertices
    if size < MIN_NUM_POLYGON_VERTICES {
        return Err(PolygonError::VertexCount);
    }

    // Must be a closed polygon
    if vertices.first() != vertices.last() {
        return Err(PolygonError::OpenPolygon);
    }

    // Each coordinate must fit within the valid range of latitude and longitude
    if vertices.iter().any(|&pt| {
        validate_pointz(
            &(PointZ {
                x: pt.longitude,
                y: pt.latitude,
                z: altitude_meters as f64,
                srid: Some(DEFAULT_SRID),
            }),
        )
        .is_err()
    }) {
        return Err(PolygonError::OutOfBounds);
    }

    Ok(PolygonZ {
        rings: vec![LineStringT {
            points: vertices
                .iter()
                .map(|vertex| PointZ {
                    x: vertex.longitude,
                    y: vertex.latitude,
                    z: altitude_meters as f64,
                    srid: Some(DEFAULT_SRID),
                })
                .collect(),
            srid: Some(DEFAULT_SRID),
        }],
        srid: Some(DEFAULT_SRID),
    })
}

/// Generate a PostGis 'Point' from a vertex
/// Each vertex must be within the valid range of latitude and longitude
pub fn point_from_vertex(vertex: &Coordinates) -> Result<Point, PointError> {
    // Each coordinate must fit within the valid range of latitude and longitude
    if vertex.latitude < -90.0
        || vertex.latitude > 90.0
        || vertex.longitude < -180.0
        || vertex.longitude > 180.0
    {
        postgis_warn!("(point_from_vertex) vertex out of bounds: {:?}", vertex);
        return Err(PointError::OutOfBounds);
    }

    Ok(Point {
        x: vertex.longitude,
        y: vertex.latitude,
        srid: Some(DEFAULT_SRID),
    })
}

/// A segment of a flight path
#[derive(Debug, ToSql)]
pub struct Segment {
    /// The geometry of the segment
    pub geom: LineStringZ,

    /// The time the segment starts
    pub time_start: DateTime<Utc>,

    /// The time the segment ends
    pub time_end: DateTime<Utc>,
}

#[derive(Debug)]
struct ExpectedResult {
    // The index of the segment
    idx: i64,

    // The geometry of the segment
    geom: LineStringZ,

    // The distance of the segment in meters
    distance_m: f64,
}

impl TryFrom<Row> for ExpectedResult {
    type Error = PostgisError;

    fn try_from(row: Row) -> Result<Self, Self::Error> {
        let idx: i64 = row.get(0);
        let geom: LineStringZ = row.get(1);
        let distance_m: f64 = row.get(2);

        Ok(ExpectedResult {
            idx,
            geom,
            distance_m,
        })
    }
}

/// Subdivides a path into time segments by length and time start/end
pub async fn segmentize(
    points: Vec<PointZ>,
    timestamp_start: DateTime<Utc>,
    timestamp_end: DateTime<Utc>,
) -> Result<Vec<Segment>, PostgisError> {
    // TODO(R5): Configurable
    /// The maximum length of a flight segment in meters
    const MAX_SEGMENT_LENGTH_M: f64 = 40.0;

    let geom = LineStringT {
        points,
        srid: Some(DEFAULT_SRID),
    };

    let stmt = "WITH segments AS (
        SELECT
            geom,
            ST_3DLength(geom) AS distance_m
        FROM ST_DumpSegments(
            (
                SELECT ST_Segmentize(
                    $1::geography,
                    $2::FLOAT
                )::geometry
            )
        )
    ) SELECT 
            ROW_NUMBER() OVER () AS idx,
            segments.geom AS geom,
            segments.distance_m AS distance_m
        FROM segments;
    "
    .to_string();

    let Some(pool) = crate::postgis::DEADPOOL_POSTGIS.get() else {
        postgis_error!("(segmentize) could not get psql pool.");
        return Err(PostgisError::Psql(PsqlError::Client));
    };

    let client = pool.get().await.map_err(|e| {
        postgis_error!(
            "(segmentize) could not get client from psql connection pool: {}",
            e
        );
        PostgisError::Psql(PsqlError::Client)
    })?;

    let mut results = client
        .query(&stmt, &[&geom, &MAX_SEGMENT_LENGTH_M])
        .await
        .map_err(|e| {
            postgis_error!("(segmentize) could not execute query: {}", e);

            PostgisError::Psql(PsqlError::Execute)
        })?
        .into_iter()
        .map(ExpectedResult::try_from)
        .collect::<Result<Vec<ExpectedResult>, PostgisError>>()?;

    results.sort_by(|a, b| a.idx.cmp(&b.idx));

    let mut cursor = timestamp_start;
    let duration = timestamp_end - timestamp_start;
    let velocity_m_s: f64 =
        results.iter().map(|r| r.distance_m).sum::<f64>() / duration.num_seconds() as f64;

    // TODO(R5): Checks for unreasonable speeds?

    let results = results
        .into_iter()
        .map(|r| {
            let segment_duration_ms = (r.distance_m / velocity_m_s) * 1000.;

            let Some(time_delta) = Duration::try_milliseconds(segment_duration_ms as i64) else {
                postgis_error!(
                    "(segmentize) could not create time delta from segment duration: {}",
                    segment_duration_ms
                );

                return Err(PostgisError::Psql(PsqlError::Execute));
            };

            let segment = Segment {
                geom: r.geom,
                time_start: cursor,
                time_end: cursor + time_delta,
            };

            cursor = segment.time_end;

            Ok(segment)
        })
        .collect::<Result<Vec<Segment>, PostgisError>>()
        .map_err(|e| {
            postgis_error!("(segmentize) could not create segment: {}", e);
            PostgisError::Psql(PsqlError::Execute)
        })?;

    postgis_debug!(
        "(segmentize) found {} segments. craft velocity {} m/s.",
        results.len(),
        velocity_m_s
    );

    Ok(results)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::{thread_rng, Rng};

    #[test]
    fn ut_point_from_vertex() {
        let mut rng = thread_rng();
        let latitude = rng.gen_range(-90.0..90.0);
        let longitude = rng.gen_range(-180.0..180.0);

        let vertex = Coordinates {
            latitude,
            longitude,
        };

        let point = point_from_vertex(&vertex).unwrap();
        assert_eq!(
            point,
            Point {
                x: longitude,
                y: latitude,
                srid: Some(DEFAULT_SRID)
            }
        );
    }

    #[test]
    fn ut_point_from_vertex_invalid() {
        let mut rng = thread_rng();
        let latitude = -90.1;
        let longitude = rng.gen_range(-180.0..180.0);

        let vertex = Coordinates {
            latitude,
            longitude,
        };

        let point = point_from_vertex(&vertex).unwrap_err();
        assert_eq!(point, PointError::OutOfBounds);

        let latitude = 0.0;
        let longitude = 180.1;

        let vertex = Coordinates {
            latitude,
            longitude,
        };
        let point = point_from_vertex(&vertex).unwrap_err();
        assert_eq!(point, PointError::OutOfBounds);
    }

    #[test]
    fn ut_polygon_from_vertices() {
        let mut rng = thread_rng();

        let mut vertices = vec![];
        for _ in 0..MIN_NUM_POLYGON_VERTICES - 1 {
            let latitude = rng.gen_range(-90.0..90.0);
            let longitude = rng.gen_range(-180.0..180.0);

            vertices.push(Coordinates {
                latitude,
                longitude,
            });
        }

        let polygon = polygon_from_vertices_z(&vertices, 122.0).unwrap_err();
        assert_eq!(polygon, PolygonError::VertexCount);

        // Close the polygon
        vertices.push(vertices.first().unwrap().clone());

        let altitude_meters = 122.0;
        let polygon = polygon_from_vertices_z(&vertices, altitude_meters).unwrap();
        let expected = PolygonZ {
            rings: vec![LineStringT {
                points: vertices
                    .iter()
                    .map(|vertex| PointZ {
                        x: vertex.longitude,
                        y: vertex.latitude,
                        z: altitude_meters as f64,
                        srid: Some(DEFAULT_SRID),
                    })
                    .collect(),
                srid: Some(DEFAULT_SRID),
            }],
            srid: Some(DEFAULT_SRID),
        };

        assert_eq!(polygon, expected);
    }

    #[test]
    fn ut_polygon_from_vertices_invalid() {
        let mut rng = thread_rng();

        let mut vertices = vec![];
        for _ in 0..MIN_NUM_POLYGON_VERTICES {
            let latitude = rng.gen_range(-90.0..90.0);
            let longitude = rng.gen_range(-180.0..180.0);

            vertices.push(Coordinates {
                latitude,
                longitude,
            });
        }

        // Do not close the polygon
        let polygon = polygon_from_vertices_z(&vertices, 100.).unwrap_err();
        assert_eq!(polygon, PolygonError::OpenPolygon);

        // Add an invalid vertex
        vertices.push(Coordinates {
            latitude: 0.0,
            longitude: 180.1,
        });

        // Close the polygon
        vertices.push(vertices.first().unwrap().clone());

        let polygon = polygon_from_vertices_z(&vertices, 100.).unwrap_err();
        assert_eq!(polygon, PolygonError::OutOfBounds);
    }

    #[test]
    fn ut_check_string() {
        // Valid
        let max_length = 20;
        let string = "test";
        let regex = &format!(r"^[0-9A-Za-z_]{{4,{max_length}}}$");
        assert!(check_string(string, regex).is_ok());

        // Invalid Length
        let string = "tes";
        assert_eq!(
            check_string(string, regex).unwrap_err(),
            StringError::Mismatch,
        );

        // Invalid Length
        let string = "T".repeat(max_length + 1);
        assert_eq!(
            check_string(&string, regex).unwrap_err(),
            StringError::Mismatch,
        );

        // Breaks Regex
        let string = "test!";
        let regex = r"^[0-9A-Za-z_]+$";
        assert_eq!(
            check_string(string, regex).unwrap_err(),
            StringError::Mismatch,
        );

        // Contains NULL
        let string = "nullTest";
        let regex = r"[0-9A-Za-z_]{3,20}";
        assert_eq!(
            check_string(string, regex).unwrap_err(),
            StringError::ContainsForbidden,
        );
    }
}
