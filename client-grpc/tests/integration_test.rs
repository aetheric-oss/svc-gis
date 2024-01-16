//! Example for writing an integration test.
//! More information: https://doc.rust-lang.org/book/testing-rust.html#integration-tests

use lib_common::grpc::get_endpoint_from_env;
use lib_common::time::Utc;
use svc_gis_client_grpc::prelude::{gis::*, *};
use uuid::Uuid;

const VERTIPORT_1_UUID: &str = "00000000-0000-0000-0000-000000000000";
const VERTIPORT_2_UUID: &str = "00000000-0000-0000-0000-000000000001";
const VERTIPORT_3_UUID: &str = "00000000-0000-0000-0000-000000000003";
const AIRCRAFT_1_UUID: &str = "00000000-0000-0000-0000-000000000002";
const AIRCRAFT_1_LABEL: &str = "Marauder";

#[tokio::test]
async fn test_add_aircraft() -> Result<(), Box<dyn std::error::Error>> {
    let (server_host, server_port) = get_endpoint_from_env("GRPC_HOST", "GRPC_PORT");
    let client = GisClient::new_client(&server_host, server_port, "compliance");

    let aircraft: Vec<(Option<String>, &str, f32, f32)> = vec![
        (
            Some(AIRCRAFT_1_UUID.to_string()),
            AIRCRAFT_1_LABEL,
            52.3746,
            4.9160036,
        ),
        (
            Some(Uuid::new_v4().to_string()),
            "Mantis",
            52.3749819,
            4.9157,
        ),
        (None, "Ghost", 52.37523, 4.9153733),
        (None, "Phantom", 52.3754, 4.9156845),
        (None, "Falcon", 52.3750703, 4.9162),
    ];

    let aircraft: Vec<AircraftPosition> = aircraft
        .iter()
        .map(|(uuid, identifier, latitude, longitude)| AircraftPosition {
            uuid: uuid.clone(),
            identifier: identifier.to_string(),
            altitude_meters: 1000.0,
            location: Some(Coordinates {
                latitude: *latitude,
                longitude: *longitude,
            }),
            timestamp_network: Some(Into::<Timestamp>::into(Utc::now())),
            timestamp_aircraft: Some(Into::<Timestamp>::into(Utc::now())),
        })
        .collect();

    let response = client
        .update_aircraft_position(UpdateAircraftPositionRequest { aircraft })
        .await?;

    println!("Response: {:?}", response);
    assert_eq!(response.into_inner().updated, true);
    Ok(())
}

#[tokio::test]
async fn test_add_vertiport() -> Result<(), Box<dyn std::error::Error>> {
    let (server_host, server_port) = get_endpoint_from_env("GRPC_HOST", "GRPC_PORT");
    let client = GisClient::new_client(&server_host, server_port, "compliance");

    let vertiports = vec![
        Vertiport {
            uuid: VERTIPORT_1_UUID.to_string(),
            vertices: vec![
                (52.3746368, 4.9163718),
                (52.3747387, 4.9162102),
                (52.3748374, 4.9163691),
                (52.3747375, 4.9165381),
                (52.3746368, 4.9163718),
            ]
            .iter()
            .map(|(x, y)| Coordinates {
                latitude: *x,
                longitude: *y,
            })
            .collect(),
            label: Some("VertiportA".to_string()),
        },
        Vertiport {
            uuid: VERTIPORT_2_UUID.to_string(),
            vertices: vec![
                (52.3751407, 4.916294),
                (52.3752201, 4.9162611),
                (52.3752627, 4.9163657),
                (52.3752107, 4.9164683),
                (52.3751436, 4.9164355),
                (52.3751407, 4.916294),
            ]
            .iter()
            .map(|(x, y)| Coordinates {
                latitude: *x,
                longitude: *y,
            })
            .collect(),
            label: Some("VertiportB".to_string()),
        },
        Vertiport {
            uuid: VERTIPORT_3_UUID.to_string(),
            vertices: vec![
                (52.3753536, 4.9157569),
                (52.3752766, 4.9157193),
                (52.375252, 4.9158829),
                (52.3753306, 4.9159232),
                (52.3753536, 4.9157569),
            ]
            .iter()
            .map(|(x, y)| Coordinates {
                latitude: *x,
                longitude: *y,
            })
            .collect(),
            label: Some("Blocker Port".to_string()),
        },
    ];

    let response = client
        .update_vertiports(UpdateVertiportsRequest { vertiports })
        .await?;

    println!("Response: {:?}", response);
    assert_eq!(response.into_inner().updated, true);
    Ok(())
}

#[tokio::test]
async fn test_add_waypoints() -> Result<(), Box<dyn std::error::Error>> {
    let (server_host, server_port) = get_endpoint_from_env("GRPC_HOST", "GRPC_PORT");
    let client = GisClient::new_client(&server_host, server_port, "compliance");

    let nodes = vec![
        ("ORANGE", 52.3745905, 4.9160036),
        ("STRAWBERRY", 52.3749819, 4.9156925),
        ("BANANA", 52.3752144, 4.9153733),
        ("LEMON", 52.3753012, 4.9156845),
        ("RASPBERRY", 52.3750703, 4.9161538),
    ];

    let waypoints = nodes
        .iter()
        .map(|(label, latitude, longitude)| Waypoint {
            label: label.to_string(),
            location: Some(Coordinates {
                latitude: *latitude,
                longitude: *longitude,
            }),
        })
        .collect();

    let response = client
        .update_waypoints(UpdateWaypointsRequest { waypoints })
        .await?;
    println!("Response: {:?}", response);
    assert_eq!(response.into_inner().updated, true);
    Ok(())
}

#[tokio::test]
async fn test_is_ready() -> Result<(), Box<dyn std::error::Error>> {
    let (server_host, server_port) = get_endpoint_from_env("GRPC_HOST", "GRPC_PORT");
    let client = GisClient::new_client(&server_host, server_port, "compliance");
    let response = client.is_ready(ReadyRequest {}).await?;
    //println!("RESPONSE={:?}", response.into_inner());
    assert_eq!(response.into_inner().ready, true);
    Ok(())
}
