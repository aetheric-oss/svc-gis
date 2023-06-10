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

    if string.contains("NULL") {
        return false;
    }

    true
}

/// Generate a PostGIS Polygon from a list of vertices
/// The first and last vertices must be equal
/// The polygon must have at least [`MIN_NUM_POLYGON_VERTICES`] vertices
/// Each vertex must be within the valid range of latitude and longitude
pub fn polygon_from_vertices(vertices: &Vec<Coordinates>) -> Result<String, PolygonError> {
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

    Ok(format!(
        "SRID=4326;POLYGON(({}))",
        vertices
            .iter()
            .map(|vertex| format!("{} {}", vertex.longitude, vertex.latitude))
            .collect::<Vec<String>>()
            .join(",")
    ))
}

/// Generate a PostGis 'Point' from a vertex
/// Each vertex must be within the valid range of latitude and longitude
pub fn point_from_vertex(vertex: &Coordinates) -> Result<String, PointError> {
    // Each coordinate must fit within the valid range of latitude and longitude
    if vertex.latitude < -90.0
        || vertex.latitude > 90.0
        || vertex.longitude < -180.0
        || vertex.longitude > 180.0
    {
        return Err(PointError::OutOfBounds);
    }

    Ok(format!(
        "SRID=4326;POINT({} {})",
        vertex.longitude, vertex.latitude
    ))
}
