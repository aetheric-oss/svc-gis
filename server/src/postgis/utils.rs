//! Common functions for PostGIS operations

use crate::grpc::server::grpc_server::Coordinates;
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

/// Check if a provided string argument is valid
pub fn check_string(string: &str, regex: &str, allowed_length: usize) -> bool {
    if string.len() > allowed_length {
        return false;
    }

    let re = regex::Regex::new(regex).unwrap();
    if !re.is_match(string) {
        return false;
    }

    if string.to_lowercase().contains("null") {
        return false;
    }

    true
}

/// Generate a PostGIS Polygon from a list of vertices
/// The first and last vertices must be equal
/// The polygon must have at least [`MIN_NUM_POLYGON_VERTICES`] vertices
/// Each vertex must be within the valid range of latitude and longitude
pub fn polygon_from_vertices(
    vertices: &Vec<Coordinates>,
) -> Result<postgis::ewkb::Polygon, PolygonError> {
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
        pt.latitude < -90.0 || pt.latitude > 90.0 || pt.longitude < -180.0 || pt.longitude > 180.0
    }) {
        return Err(PolygonError::OutOfBounds);
    }

    Ok(postgis::ewkb::Polygon {
        rings: vec![postgis::ewkb::LineString {
            points: vertices
                .iter()
                .map(|vertex| postgis::ewkb::Point {
                    x: vertex.longitude as f64,
                    y: vertex.latitude as f64,
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
        x: vertex.longitude as f64,
        y: vertex.latitude as f64,
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
                x: longitude as f64,
                y: latitude as f64,
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

        let polygon = polygon_from_vertices(&vertices).unwrap_err();
        assert_eq!(polygon, PolygonError::VertexCount);

        // Close the polygon
        vertices.push(vertices.first().unwrap().clone());

        let vertex_str = vertices
            .iter()
            .map(|vertex| format!("{:.6} {:.6}", vertex.longitude, vertex.latitude))
            .collect::<Vec<String>>()
            .join(",");

        let polygon = polygon_from_vertices(&vertices).unwrap();
        let expected = postgis::ewkb::Polygon {
            rings: vec![postgis::ewkb::LineString {
                points: vertices
                    .iter()
                    .map(|vertex| postgis::ewkb::Point {
                        x: vertex.longitude as f64,
                        y: vertex.latitude as f64,
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
        let polygon = polygon_from_vertices(&vertices).unwrap_err();
        assert_eq!(polygon, PolygonError::OpenPolygon);

        // Add an invalid vertex
        vertices.push(Coordinates {
            latitude: 0.0,
            longitude: 180.1,
        });

        // Close the polygon
        vertices.push(vertices.first().unwrap().clone());

        let polygon = polygon_from_vertices(&vertices).unwrap_err();
        assert_eq!(polygon, PolygonError::OutOfBounds);
    }

    #[test]
    fn ut_check_string() {
        // Valid
        let string = "test";
        let regex = r"^[a-zA-Z0-9_]+$";
        let allowed_length = 10;

        assert!(check_string(string, regex, allowed_length));

        // Invalid Length
        let string = "test";
        let regex = r"^[a-zA-Z0-9_]+$";
        let allowed_length = 3;
        assert!(!check_string(string, regex, allowed_length));

        // Breaks Regex
        let string = "test!";
        let regex = r"^[a-zA-Z0-9_]+$";
        let allowed_length = 10;
        assert!(!check_string(string, regex, allowed_length));

        // Contains NULL
        let string = "nullTest";
        let regex = r"^[a-zA-Z0-9_]+$";
        let allowed_length = 10;
        assert!(!check_string(string, regex, allowed_length));
    }
}
