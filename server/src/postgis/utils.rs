//! Common functions for PostGIS operations

use crate::grpc::server::grpc_server::Coordinates;
use geo::algorithm::haversine_distance::HaversineDistance;
use geo::point;
use postgis::ewkb::PointZ;
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

/// Generate a PostGIS Polygon from a list of vertices
/// The first and last vertices must be equal
/// The polygon must have at least [`MIN_NUM_POLYGON_VERTICES`] vertices
/// Each vertex must be within the valid range of latitude and longitude
pub fn polygon_from_vertices_z(
    vertices: &[Coordinates],
    altitude_meters: f32,
) -> Result<postgis::ewkb::PolygonZ, PolygonError> {
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
                srid: Some(4326),
            }),
        )
        .is_err()
    }) {
        return Err(PolygonError::OutOfBounds);
    }

    Ok(postgis::ewkb::PolygonZ {
        rings: vec![postgis::ewkb::LineStringT {
            points: vertices
                .iter()
                .map(|vertex| PointZ {
                    x: vertex.longitude,
                    y: vertex.latitude,
                    z: altitude_meters as f64,
                    srid: Some(4326),
                })
                .collect(),
            srid: Some(4326),
        }],
        srid: Some(4326),
    })
}

/// Generate a PostGis 'Point' from a vertex
/// Each vertex must be within the valid range of latitude and longitude
pub fn point_from_vertex(vertex: &Coordinates) -> Result<postgis::ewkb::Point, PointError> {
    // Each coordinate must fit within the valid range of latitude and longitude
    if vertex.latitude < -90.0
        || vertex.latitude > 90.0
        || vertex.longitude < -180.0
        || vertex.longitude > 180.0
    {
        return Err(PointError::OutOfBounds);
    }

    Ok(postgis::ewkb::Point {
        x: vertex.longitude,
        y: vertex.latitude,
        srid: Some(4326),
    })
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
            postgis::ewkb::Point {
                x: longitude,
                y: latitude,
                srid: Some(4326)
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
        let expected = postgis::ewkb::PolygonZ {
            rings: vec![postgis::ewkb::LineStringT {
                points: vertices
                    .iter()
                    .map(|vertex| PointZ {
                        x: vertex.longitude,
                        y: vertex.latitude,
                        z: altitude_meters as f64,
                        srid: Some(4326),
                    })
                    .collect(),
                srid: Some(4326),
            }],
            srid: Some(4326),
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
