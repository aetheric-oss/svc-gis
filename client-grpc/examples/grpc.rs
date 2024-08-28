//! gRPC client implementation
//! Helps to use https://www.keene.edu/campus/maps/tool/ to create polygons on a map

use geo::{polygon, Centroid};
use lib_common::grpc::get_endpoint_from_env;
use lib_common::time::{DateTime, Duration, Utc};
use svc_gis_client_grpc::prelude::{gis::*, *};

const VERTIPORT_1_ID: &str = "Kamino";
const VERTIPORT_2_ID: &str = "Bespin";
const VERTIPORT_3_ID: &str = "Coruscant";
const AIRCRAFT_1_ID: &str = "Marauder";

async fn add_vertiports(client: &GisClient) -> Result<(), Box<dyn std::error::Error>> {
    println!("\n\u{1F6EB} Add Vertiports");
    let vertiports = vec![
        Vertiport {
            identifier: VERTIPORT_1_ID.to_string(),
            altitude_meters: 10.0,
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
            timestamp_network: Some(Utc::now().into()),
        },
        Vertiport {
            identifier: VERTIPORT_2_ID.to_string(),
            altitude_meters: 10.0,
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
            timestamp_network: Some(Utc::now().into()),
        },
        Vertiport {
            identifier: VERTIPORT_3_ID.to_string(),
            altitude_meters: 10.0,
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
            timestamp_network: Some(Utc::now().into()),
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
        .map(|(identifier, latitude, longitude)| Waypoint {
            identifier: identifier.to_string(),
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

async fn add_aircraft(connection: &mut redis::Connection) -> Result<(), ()> {
    println!("\n\u{1F681} Add Aircraft");

    let sample: Vec<(&str, f64, f64, f64)> = vec![
        (AIRCRAFT_1_ID, 52.3746, 4.9160036, 100.),
        ("Mantis", 52.3749819, 4.9157, 200.),
        ("Ghost", 52.37523, 4.9153733, 50.),
        ("Phantom", 52.3754, 4.9156845, 20.),
        ("Falcon", 52.3750703, 4.9162, 30.),
    ];

    let aircraft: Vec<AircraftPosition> = sample
        .iter()
        .map(
            |(identifier, latitude, longitude, altitude_meters)| AircraftPosition {
                identifier: identifier.to_string(),
                position: Position {
                    latitude: *latitude,
                    longitude: *longitude,
                    altitude_meters: *altitude_meters,
                },
                timestamp_network: Utc::now(),
                timestamp_asset: None,
            },
        )
        .collect();

    let mut pipe = redis::pipe();
    aircraft.iter().for_each(|aircraft| {
        (&mut pipe).rpush(
            REDIS_KEY_AIRCRAFT_POSITION,
            serde_json::to_vec(&aircraft).unwrap(),
        );
    });

    pipe.query::<()>(connection).unwrap();

    let aircraft: Vec<AircraftId> = sample
        .iter()
        .map(|(identifier, _, _, _)| AircraftId {
            identifier: Some(identifier.to_string()),
            session_id: None,
            aircraft_type: AircraftType::Rotorcraft,
            timestamp_network: Utc::now(),
            timestamp_asset: None,
        })
        .collect();

    let mut pipe = redis::pipe();
    aircraft.iter().for_each(|aircraft| {
        (&mut pipe).rpush(
            REDIS_KEY_AIRCRAFT_ID,
            serde_json::to_vec(&aircraft).unwrap(),
        );
    });

    pipe.query::<()>(connection).unwrap();

    let aircraft: Vec<AircraftVelocity> = sample
        .iter()
        .map(|(identifier, _, _, _)| AircraftVelocity {
            identifier: identifier.to_string(),
            velocity_horizontal_air_mps: None,
            velocity_horizontal_ground_mps: 100.0,
            velocity_vertical_mps: 10.0,
            track_angle_degrees: 10.0,
            timestamp_network: Utc::now(),
            timestamp_asset: None,
        })
        .collect();

    let mut pipe = redis::pipe();
    aircraft.iter().for_each(|aircraft| {
        (&mut pipe).rpush(
            REDIS_KEY_AIRCRAFT_VELOCITY,
            serde_json::to_vec(&aircraft).unwrap(),
        );
    });

    pipe.query::<()>(connection).unwrap();

    Ok(())
}

async fn add_flight_paths(client: &GisClient) -> Result<(), ()> {
    println!("\n\u{1F681} Add Flights");

    let path = vec![
        PointZ {
            latitude: 52.3746,
            longitude: 4.9160036,
            altitude_meters: 100.,
        },
        PointZ {
            latitude: 52.3749819,
            longitude: 4.9157,
            altitude_meters: 200.,
        },
        PointZ {
            latitude: 52.37523,
            longitude: 4.9153733,
            altitude_meters: 50.,
        },
    ];

    let sample: Vec<&str> = vec![AIRCRAFT_1_ID, "Mantis", "Ghost", "Phantom"]; // leave off falcon

    let items: Vec<UpdateFlightPathRequest> = sample
        .into_iter()
        .enumerate()
        .map(|(i, aircraft_identifier)| UpdateFlightPathRequest {
            flight_identifier: Some(format!("FLIGHT-{}", i)),
            aircraft_identifier: Some(aircraft_identifier.to_string()),
            path: path.clone(),
            timestamp_start: Some(Utc::now().into()),
            timestamp_end: Some((Utc::now() + Duration::try_minutes(20).unwrap()).into()),
            simulated: false,
            aircraft_type: AircraftType::Rotorcraft as i32,
        })
        .collect();

    for item in items {
        client.update_flight_path(item).await.map_err(|_| ())?;
    }

    Ok(())
}

async fn best_path_flight_avoidance(
    _connection: &mut redis::Connection,
    client: &GisClient,
) -> Result<(), Box<dyn std::error::Error>> {
    // Add two vertiports specifically for this test
    // Place somewhere far from the others in this test
    // No waypoints, only direct path between vertiports

    const DEFAULT_ALTITUDE: f64 = 10.0;

    const ALKMAAR_1_ID: &str = "ALKMAAR_1";
    let alkmaar_1_polygon = geo::polygon![
        (x: 4.713655228916873, y: 52.63040456142831),
        (x: 4.713655228916873, y: 52.63006644856257),
        (x: 4.714239138046231, y: 52.63006644856257),
        (x: 4.714239138046231, y: 52.63040456142831),
        (x: 4.713655228916873, y: 52.63040456142831)
    ];

    let vertices = alkmaar_1_polygon
        .exterior()
        .points()
        .map(|pt| Coordinates {
            latitude: pt.y(),
            longitude: pt.x(),
        })
        .collect();

    let alkmaar_1 = Vertiport {
        identifier: ALKMAAR_1_ID.to_string(),
        altitude_meters: DEFAULT_ALTITUDE as f32,
        vertices,
        label: Some("Alkmaar 1".to_string()),
        timestamp_network: Some(Utc::now().into()),
    };

    const ALKMAAR_2_ID: &str = "ALKMAAR_2";
    let alkmaar_2_polygon = geo::polygon![
        (x: 4.7183918814820895, y: 52.63404933036138),
        (x: 4.7183918814820895, y: 52.633870374451455),
        (x: 4.718710818704621, y: 52.633870374451455),
        (x: 4.718710818704621, y: 52.63404933036138),
        (x: 4.7183918814820895, y: 52.63404933036138)
    ];

    let vertices = alkmaar_2_polygon
        .exterior()
        .points()
        .map(|pt| Coordinates {
            latitude: pt.y(),
            longitude: pt.x(),
        })
        .collect();

    let alkmaar_2 = Vertiport {
        identifier: ALKMAAR_2_ID.to_string(),
        altitude_meters: DEFAULT_ALTITUDE as f32,
        vertices,
        label: Some("Alkmaar 2".to_string()),
        timestamp_network: Some(Utc::now().into()),
    };

    let vertiports = vec![alkmaar_1.clone(), alkmaar_2.clone()];
    let _ = client
        .update_vertiports(UpdateVertiportsRequest { vertiports })
        .await?;

    let time_start: DateTime<Utc> = Utc::now();
    let time_end: DateTime<Utc> = Utc::now() + Duration::try_minutes(15).unwrap();

    // Best Path Request
    println!("\n\u{1F426} Best Path With Zero Other Flight Paths Prior");
    let request = BestPathRequest {
        origin_identifier: ALKMAAR_1_ID.to_string(),
        target_identifier: ALKMAAR_2_ID.to_string(),
        origin_type: NodeType::Vertiport as i32,
        target_type: NodeType::Vertiport as i32,
        time_start: Some(time_start.clone().into()),
        time_end: Some(time_end.clone().into()),
        limit: 1,
    };

    let response = client.best_path(request).await?.into_inner();
    let Some(path) = response.paths.first() else {
        panic!("No path found.");
    };

    println!("(best_path_flight_avoidance) Found flight path at Alkmaar:");
    display_paths(&vec![path.clone()]);

    //
    // Insert new flight plan for the same time and path, but at a different altitude
    //  PostGIS does a 2D comparison of the flight paths first
    //  If the 2D representation of the flight paths intersect/come close, then the
    //  3D comparison is done to see if the flight paths are at different altitudes
    let path = [
        alkmaar_1_polygon.centroid().unwrap(),
        alkmaar_2_polygon.centroid().unwrap(),
    ]
    .into_iter()
    .map(|pt| PointZ {
        latitude: pt.y(),
        longitude: pt.x(),
        altitude_meters: 200.0,
    })
    .collect::<Vec<PointZ>>();

    let request = UpdateFlightPathRequest {
        flight_identifier: Some("FLIGHT-Y".to_string()),
        aircraft_identifier: Some("Razor Crest".to_string()),
        path,
        timestamp_start: Some(time_start.into()),
        timestamp_end: Some(time_end.into()),
        simulated: false,
        aircraft_type: AircraftType::Rotorcraft as i32,
    };

    let _ = client.update_flight_path(request).await?.into_inner();

    // Best Path Request
    println!("\n\u{1F426} Best Path With Prior Flight Path @ Different Altitude (No-Intersect)");
    let request = BestPathRequest {
        origin_identifier: ALKMAAR_1_ID.to_string(),
        target_identifier: ALKMAAR_2_ID.to_string(),
        origin_type: NodeType::Vertiport as i32,
        target_type: NodeType::Vertiport as i32,
        time_start: Some(time_start.clone().into()),
        time_end: Some(time_end.clone().into()),
        limit: 1,
    };

    let response = client.best_path(request).await?.into_inner();
    let Some(path) = response.paths.first() else {
        panic!("No path found.");
    };

    println!("(best_path_flight_avoidance) Found flight path at Alkmaar:");
    display_paths(&vec![path.clone()]);

    //
    // Insert new flight plan
    //
    let path = [
        alkmaar_1_polygon.centroid().unwrap(),
        alkmaar_2_polygon.centroid().unwrap(),
    ]
    .into_iter()
    .map(|pt| PointZ {
        latitude: pt.y(),
        longitude: pt.x(),
        altitude_meters: DEFAULT_ALTITUDE as f32,
    })
    .collect::<Vec<PointZ>>();

    let request = UpdateFlightPathRequest {
        flight_identifier: Some("AETH12345".to_string()),
        aircraft_identifier: Some("AETH-CRAFT-X".to_string()),
        path,
        timestamp_start: Some(time_start.into()),
        timestamp_end: Some(time_end.into()),
        simulated: false,
        aircraft_type: AircraftType::Rotorcraft as i32,
    };

    let _ = client.update_flight_path(request).await?.into_inner();

    //
    // Try Best Path again during same time as other flight, should be none
    //
    println!("\n\u{1F426} Best Path With Interrupting Flight");
    let request = BestPathRequest {
        origin_identifier: ALKMAAR_1_ID.to_string(),
        target_identifier: ALKMAAR_2_ID.to_string(),
        origin_type: NodeType::Vertiport as i32,
        target_type: NodeType::Vertiport as i32,
        time_start: Some(time_start.clone().into()),
        time_end: Some(time_end.clone().into()),
        limit: 1,
    };

    let response = client.best_path(request).await?.into_inner();
    match response.paths.first() {
        Some(path) => {
            println!("Path found when it should not have been possible.");
            display_paths(&vec![path.clone()]);
            panic!()
        }
        None => println!("No path found, as expected."),
    };

    //
    // Best Path Request AFTER
    //
    println!("\n\u{1F426} Best Path AFTER Interrupting Flight");
    let request = BestPathRequest {
        origin_identifier: ALKMAAR_1_ID.to_string(),
        target_identifier: ALKMAAR_2_ID.to_string(),
        origin_type: NodeType::Vertiport as i32,
        target_type: NodeType::Vertiport as i32,
        time_start: Some((time_end.clone() + Duration::try_seconds(1).unwrap()).into()),
        time_end: Some((time_end.clone() + Duration::try_minutes(1).unwrap()).into()),
        limit: 1,
    };

    let response = client.best_path(request).await?.into_inner();
    let Some(path) = response.paths.first() else {
        panic!("No path found.");
    };

    println!("(best_path_flight_avoidance) Found flight path at Alkmaar:");
    display_paths(&vec![path.clone()]);

    //
    // Try Best Path again during same time as other flight, with offset of time
    //
    println!("\n\u{1F426} Best Path With Identical Flight at Time Offset");
    let request = BestPathRequest {
        origin_identifier: ALKMAAR_1_ID.to_string(),
        target_identifier: ALKMAAR_2_ID.to_string(),
        origin_type: NodeType::Vertiport as i32,
        target_type: NodeType::Vertiport as i32,
        time_start: Some((time_end - Duration::try_seconds(2).unwrap()).into()),
        time_end: Some((time_end + Duration::try_minutes(13).unwrap()).into()),
        limit: 1,
    };

    let response = client.best_path(request).await?.into_inner();
    let Some(path) = response.paths.first() else {
        panic!("No path found.");
    };

    println!("(best_path_flight_avoidance) Found flight path at Alkmaar:");
    display_paths(&vec![path.clone()]);

    Ok(())
}

/// Get active flights
async fn get_flights(client: &GisClient) -> Result<(), Box<dyn std::error::Error>> {
    {
        println!("\n\u{1F426} Get Active Flights");
        let time_start: Timestamp = (Utc::now() - Duration::try_seconds(30).unwrap()).into();
        let time_end: Timestamp = Utc::now().into();
        let request = GetFlightsRequest {
            window_min_x: 4.915,
            window_min_y: 52.374,
            window_max_x: 4.917,
            window_max_y: 52.376,
            time_start: Some(time_start),
            time_end: Some(time_end),
        };

        let response = client.get_flights(request).await?.into_inner();

        println!("RESPONSE={:?}", response);
        if response.flights.is_empty() {
            panic!("No flights found.")
        }
    }

    Ok(())
}

async fn best_paths(client: &GisClient) -> Result<(), Box<dyn std::error::Error>> {
    // Best Path Without No-Fly Zone
    {
        println!("\n\u{1F426} Best Path WITHOUT Temporary No-Fly Zone");
        let time_start: Timestamp = Utc::now().into();
        let time_end: Timestamp = (Utc::now() + Duration::try_hours(2).unwrap()).into();
        let request = BestPathRequest {
            origin_identifier: VERTIPORT_1_ID.to_string(),
            target_identifier: VERTIPORT_2_ID.to_string(),
            origin_type: NodeType::Vertiport as i32,
            target_type: NodeType::Vertiport as i32,
            time_start: Some(time_start),
            time_end: Some(time_end),
            limit: 1,
        };

        let response = client.best_path(request).await?.into_inner();

        println!("RESPONSE={:?}", response);
        display_paths(&response.paths);
    }

    let no_fly_start_time = Utc::now();
    let no_fly_end_time = Utc::now() + Duration::try_hours(2).unwrap();

    // Update No-Fly Zones
    {
        println!("\n\u{26D4} Add No-Fly Zones");
        let mut zones: Vec<Zone> = vec![];

        // No Fly 1
        let vertices: Vec<(f64, f64)> = vec![
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
        zones.push(Zone {
            identifier: "NL-NFZ-01".to_string(),
            zone_type: ZoneType::Restriction as i32,
            altitude_meters_max: 1000.0,
            altitude_meters_min: 0.0,
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

        zones.push(Zone {
            identifier: "NL-NFZ-02".to_string(),
            zone_type: ZoneType::Restriction as i32,
            altitude_meters_max: 1000.0,
            altitude_meters_min: 0.0,
            vertices,
            time_start: None,
            time_end: None,
        });

        let response = client.update_zones(UpdateZonesRequest { zones }).await?;

        println!("RESPONSE={:?}", response.into_inner());
    }

    // Best Path During Temporary No-Fly Zone
    {
        println!("\n\u{26D4}\u{1F681} Best Path DURING Temporary No-Fly Zone");
        let time_start: Timestamp = no_fly_start_time.into();
        let time_end: Timestamp = (no_fly_start_time + Duration::try_hours(1).unwrap()).into();
        let request = BestPathRequest {
            origin_identifier: VERTIPORT_1_ID.to_string(),
            target_identifier: VERTIPORT_2_ID.to_string(),
            origin_type: NodeType::Vertiport as i32,
            target_type: NodeType::Vertiport as i32,
            time_start: Some(time_start),
            time_end: Some(time_end),
            limit: 1,
        };

        let mut response = client.best_path(request).await?.into_inner();

        println!("RESPONSE={:?}", response);
        display_paths(&response.paths);

        let expected = 3;
        if response.paths.pop().unwrap().path.len() != expected {
            panic!("Expected {expected} paths, got {}", response.paths.len());
        }
    }

    // Best Path After Temporary No-Fly Zone
    {
        println!("\n\u{1F681} Best Path AFTER Temporary No-Fly Zone Expires");
        let time_start: Timestamp = (no_fly_end_time + Duration::try_seconds(1).unwrap()).into();
        let time_end: Timestamp = (no_fly_end_time + Duration::try_hours(1).unwrap()).into();
        let request = BestPathRequest {
            origin_identifier: VERTIPORT_1_ID.to_string(),
            target_identifier: VERTIPORT_2_ID.to_string(),
            origin_type: NodeType::Vertiport as i32,
            target_type: NodeType::Vertiport as i32,
            time_start: Some(time_start),
            time_end: Some(time_end),
            limit: 1,
        };

        let response = client.best_path(request).await?.into_inner();

        println!("RESPONSE={:?}", response);
        display_paths(&response.paths);
    }

    // Best Path From Aircraft
    {
        println!("\n\u{1F681} Best Path From Aircraft during TFR");
        let time_start: Timestamp = no_fly_start_time.into();
        let time_end: Timestamp = (no_fly_start_time + Duration::try_hours(1).unwrap()).into();
        let request = BestPathRequest {
            origin_identifier: AIRCRAFT_1_ID.to_string(),
            target_identifier: VERTIPORT_2_ID.to_string(),
            origin_type: NodeType::Aircraft as i32,
            target_type: NodeType::Vertiport as i32,
            time_start: Some(time_start),
            time_end: Some(time_end),
            limit: 5,
        };

        let response = client.best_path(request).await?.into_inner();

        println!("RESPONSE={:?}", response);
        display_paths(&response.paths);
    }

    Ok(())
}

fn display_paths(paths: &[Path]) {
    println!("\x1b[33;3m{} paths found.\x1b[0m", paths.len());

    for (idx, path) in paths.iter().enumerate() {
        println!("\nPath {idx}: ({} meters):", path.distance_meters);
        for node in &path.path {
            println!("\t{}: {:?}", node.index, node);
        }
    }
}

/// Example svc-gis-client-grpc
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let Ok(redis_client) = redis::Client::open(std::env::var("REDIS__URL").unwrap()) else {
        panic!("Could not create redis client.");
    };

    let Ok(mut connection) = redis_client.get_connection() else {
        panic!("Could not get redis connection.");
    };

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

    add_aircraft(&mut connection).await.unwrap();
    add_flight_paths(&client).await.unwrap();
    std::thread::sleep(std::time::Duration::from_secs(1));
    get_flights(&client).await?;
    add_vertiports(&client).await?;
    add_waypoints(&client).await?;
    best_paths(&client).await?;
    best_path_flight_avoidance(&mut connection, &client).await?;

    Ok(())
}
