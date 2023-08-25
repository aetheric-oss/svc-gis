//! gRPC client implementation
//! Helps to use https://www.keene.edu/campus/maps/tool/ to create polygons on a map

use chrono::{Duration, Utc};
use lib_common::grpc::get_endpoint_from_env;
use svc_gis_client_grpc::prelude::{gis::*, *};
use uuid::Uuid;

const VERTIPORT_1_UUID: &str = "00000000-0000-0000-0000-000000000000";
const VERTIPORT_2_UUID: &str = "00000000-0000-0000-0000-000000000001";
const VERTIPORT_3_UUID: &str = "00000000-0000-0000-0000-000000000003";
const AIRCRAFT_1_UUID: &str = "00000000-0000-0000-0000-000000000002";
const AIRCRAFT_1_LABEL: &str = "Marauder";

async fn add_vertiports(client: &GisClient) -> Result<(), Box<dyn std::error::Error>> {
    println!("\n\u{1F6EB} Add Vertiports");
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

    println!("RESPONSE={:?}", response.into_inner());

    Ok(())
}

async fn add_waypoints(client: &GisClient) -> Result<(), Box<dyn std::error::Error>> {
    println!("\n\u{1F4CD} Add Waypoints");
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

    println!("RESPONSE={:?}", response.into_inner());

    Ok(())
}

async fn add_aircraft(client: &GisClient) -> Result<(), Box<dyn std::error::Error>> {
    println!("\n\u{1F681} Add Aircraft");

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
        .map(|(uuid, callsign, latitude, longitude)| AircraftPosition {
            uuid: uuid.clone(),
            callsign: callsign.to_string(),
            altitude_meters: 1000.0,
            location: Some(Coordinates {
                latitude: *latitude,
                longitude: *longitude,
            }),
            time: Some(Into::<Timestamp>::into(Utc::now())),
        })
        .collect();

    let response = client
        .update_aircraft_position(UpdateAircraftPositionRequest { aircraft })
        .await?;

    println!("RESPONSE={:?}", response.into_inner());

    Ok(())
}

/// Example svc-gis-client-grpc
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let (host, port) = get_endpoint_from_env("SERVER_HOSTNAME", "SERVER_PORT_GRPC");
    let client = GisClient::new_client(&host, port, "gis");
    println!("Client created");
    println!(
        "NOTE: Ensure the server is running on {} or this example will fail.",
        client.get_address()
    );

    {
        println!("\n\u{1F44D} Ready Check");
        let response = client.is_ready(ReadyRequest {}).await?.into_inner();

        println!("RESPONSE={:?}", response);
        assert_eq!(response.ready, true);
    }

    add_aircraft(&client).await?;
    add_vertiports(&client).await?;
    add_waypoints(&client).await?;

    // Best Path Without No-Fly Zone
    {
        println!("\n\u{1F426} Best Path WITHOUT Temporary No-Fly Zone");
        let time_start: Timestamp = Utc::now().into();
        let time_end: Timestamp = (Utc::now() + Duration::hours(2)).into();
        let request = BestPathRequest {
            node_start_id: VERTIPORT_1_UUID.to_string(),
            node_uuid_end: VERTIPORT_2_UUID.to_string(),
            start_type: NodeType::Vertiport as i32,
            time_start: Some(time_start),
            time_end: Some(time_end),
        };

        let response = client.best_path(request).await?.into_inner();

        println!("RESPONSE={:?}", response);
        println!(
            "\x1b[33;3m{} segment(s) in path.\x1b[0m",
            response.segments.len()
        );
    }

    let no_fly_start_time = Utc::now();
    let no_fly_end_time = Utc::now() + Duration::hours(2);

    // Update No-Fly Zones
    {
        println!("\n\u{26D4} Add No-Fly Zones");
        let mut zones: Vec<NoFlyZone> = vec![];

        // No Fly 1
        let vertices: Vec<(f32, f32)> = vec![
            (52.3751734, 4.9158481),
            (52.3750752, 4.9157998),
            (52.3749409, 4.9164569),
            (52.3751047, 4.9164999),
            (52.3751734, 4.9158481),
        ];

        let vertices: Vec<Coordinates> = vertices
            .iter()
            .map(|(x, y)| Coordinates {
                latitude: *x,
                longitude: *y,
            })
            .collect();

        let time_start: Timestamp = no_fly_start_time.into();
        let time_end: Timestamp = no_fly_end_time.into();
        zones.push(NoFlyZone {
            label: "NL-NFZ-01".to_string(),
            vertices,
            time_start: Some(time_start),
            time_end: Some(time_end),
        });

        // No Fly 2
        let vertices = vec![
            (52.3743089, 4.9159741),
            (52.3749147, 4.9169827),
            (52.3751309, 4.9165696),
            (52.3755009, 4.9166715),
            (52.3751309, 4.9191499),
            (52.3730774, 4.9166822),
            (52.3732215, 4.9143541),
            (52.3749769, 4.9132517),
            (52.3758464, 4.9145097),
            (52.3757465, 4.9152178),
            (52.3751456, 4.9149576),
            (52.3748934, 4.9155074),
            (52.3743089, 4.9159741),
        ];

        let vertices: Vec<Coordinates> = vertices
            .iter()
            .map(|(x, y)| Coordinates {
                latitude: *x,
                longitude: *y,
            })
            .collect();

        zones.push(NoFlyZone {
            label: "NL-NFZ-02".to_string(),
            vertices,
            time_start: None,
            time_end: None,
        });

        let response = client
            .update_no_fly_zones(UpdateNoFlyZonesRequest { zones })
            .await?;

        println!("RESPONSE={:?}", response.into_inner());
    }

    // Best Path During Temporary No-Fly Zone
    {
        println!("\n\u{26D4}\u{1F681} Best Path DURING Temporary No-Fly Zone");
        let time_start: Timestamp = no_fly_start_time.into();
        let time_end: Timestamp = (no_fly_start_time + Duration::hours(1)).into();
        let request = BestPathRequest {
            node_start_id: VERTIPORT_1_UUID.to_string(),
            node_uuid_end: VERTIPORT_2_UUID.to_string(),
            start_type: NodeType::Vertiport as i32,
            time_start: Some(time_start),
            time_end: Some(time_end),
        };

        let response = client.best_path(request).await?.into_inner();

        println!("RESPONSE={:?}", response);
        println!(
            "\x1b[33;3m{} segment(s) in path.\x1b[0m",
            response.segments.len()
        );
    }

    // Best Path After Temporary No-Fly Zone
    {
        println!("\n\u{1F681} Best Path AFTER Temporary No-Fly Zone Expires");
        let time_start: Timestamp = (no_fly_end_time + Duration::seconds(1)).into();
        let time_end: Timestamp = (no_fly_end_time + Duration::hours(1)).into();
        let request = BestPathRequest {
            node_start_id: VERTIPORT_1_UUID.to_string(),
            node_uuid_end: VERTIPORT_2_UUID.to_string(),
            start_type: NodeType::Vertiport as i32,
            time_start: Some(time_start),
            time_end: Some(time_end),
        };

        let response = client.best_path(request).await?.into_inner();

        println!("RESPONSE={:?}", response);
        println!(
            "\x1b[33;3m{} segment(s) in path.\x1b[0m",
            response.segments.len()
        );
    }

    // Best Path From Aircraft
    {
        println!("\n\u{1F681} Best Path From Aircraft during TFR");
        let time_start: Timestamp = no_fly_start_time.into();
        let time_end: Timestamp = (no_fly_start_time + Duration::hours(1)).into();
        let request = BestPathRequest {
            node_start_id: AIRCRAFT_1_LABEL.to_string(),
            node_uuid_end: VERTIPORT_2_UUID.to_string(),
            start_type: NodeType::Aircraft as i32,
            time_start: Some(time_start),
            time_end: Some(time_end),
        };

        let response = client.best_path(request).await?.into_inner();

        println!("RESPONSE={:?}", response);
        println!(
            "\x1b[33;3m{} segment(s) in path.\x1b[0m",
            response.segments.len()
        );
    }

    // Nearest Neighbor to Vertiport
    {
        println!("\n\u{1F3E0} Nearest Vertiport Neighbors to Vertiport");
        let request = NearestNeighborRequest {
            start_node_id: VERTIPORT_1_UUID.to_string(),
            start_type: NodeType::Vertiport as i32,
            end_type: NodeType::Vertiport as i32,
            limit: 10,
            max_range_meters: 3000.0,
        };

        let response = client.nearest_neighbors(request).await?.into_inner();

        println!("RESPONSE={:?}", response);
        println!(
            "\x1b[33;3m{} nearest neighbors(s).\x1b[0m",
            response.distances.len()
        );
    }

    // Nearest Neighbor to Aircraft
    {
        println!("\n\u{1F3E0} Nearest Vertiport Neighbors to Aircraft");
        let request = NearestNeighborRequest {
            start_node_id: AIRCRAFT_1_LABEL.to_string(),
            start_type: NodeType::Aircraft as i32,
            end_type: NodeType::Vertiport as i32,
            limit: 10,
            max_range_meters: 3000.0,
        };

        let response = client.nearest_neighbors(request).await?.into_inner();

        println!("RESPONSE={:?}", response);
        println!(
            "\x1b[33;3m{} nearest neighbors(s).\x1b[0m",
            response.distances.len()
        );
    }

    Ok(())
}
