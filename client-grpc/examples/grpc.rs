//! gRPC client implementation

use std::env;
#[allow(unused_qualifications, missing_docs)]
use svc_gis_client_grpc::client::{
    rpc_service_client::RpcServiceClient, BestPathRequest, Coordinates, NoFlyZone, Node, NodeType,
    ReadyRequest, UpdateNoFlyZonesRequest, UpdateNodesRequest,
};

use chrono::{Duration, Utc};
use lib_common::time::datetime_to_timestamp;

const VERTIPORT_1_UUID: &str = "00000000-0000-0000-0000-000000000000";
const VERTIPORT_2_UUID: &str = "00000000-0000-0000-0000-000000000001";

/// Provide endpoint url to use
pub fn get_grpc_endpoint() -> String {
    //parse socket address from env variable or take default value
    let address = match env::var("SERVER_HOSTNAME") {
        Ok(val) => val,
        Err(_) => "localhost".to_string(), // default value
    };

    let port = match env::var("SERVER_PORT_GRPC") {
        Ok(val) => val,
        Err(_) => "50008".to_string(), // default value
    };

    format!("http://{}:{}", address, port)
}

/// Example svc-gis-client-grpc
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let grpc_endpoint = get_grpc_endpoint();

    println!(
        "NOTE: Ensure the server is running on {} or this example will fail.",
        grpc_endpoint
    );

    let mut client = RpcServiceClient::connect(grpc_endpoint).await?;

    // Update Nodes
    {
        println!("\n\u{1F4CD} Add Nodes");

        let nodes = vec![
            (52.3745905, 4.9160036, None),
            (52.3749819, 4.9156925, None),
            (52.3752144, 4.9153733, None),
            (52.3753012, 4.9156845, None),
            (52.3750703, 4.9161538, None),
            (
                52.374740703179484,
                4.916379271589524,
                Some(VERTIPORT_1_UUID),
            ),
            (
                52.375183975669685,
                4.916365467571953,
                Some(VERTIPORT_2_UUID),
            ),
        ];

        let nodes = nodes
            .iter()
            .map(|(x, y, vertiport)| Node {
                uuid: match vertiport {
                    None => uuid::Uuid::new_v4().to_string(),
                    Some(s) => s.to_string(),
                },
                location: Some(Coordinates {
                    latitude: *x,
                    longitude: *y,
                }),
                node_type: match vertiport {
                    None => NodeType::Waypoint as i32,
                    _ => NodeType::Vertiport as i32,
                },
            })
            .collect();

        let request = tonic::Request::new(UpdateNodesRequest { nodes });
        let response = client.update_nodes(request).await?;

        println!("RESPONSE={:?}", response.into_inner());
    }

    // Best Path Without No-Fly Zone
    {
        println!("\n\u{1F426} Best Path WITHOUT Temporary No-Fly Zone");
        let request = tonic::Request::new(BestPathRequest {
            node_uuid_start: VERTIPORT_1_UUID.to_string(),
            node_uuid_end: VERTIPORT_2_UUID.to_string(),
            time_start: datetime_to_timestamp(&Utc::now()),
            time_end: datetime_to_timestamp(&(Utc::now() + Duration::hours(2))),
        });

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

        zones.push(NoFlyZone {
            label: "NL-NFZ-01".to_string(),
            vertices,
            time_start: datetime_to_timestamp(&no_fly_start_time),
            time_end: datetime_to_timestamp(&no_fly_end_time),
            vertiport_id: None,
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
            vertiport_id: None,
        });

        let request = tonic::Request::new(UpdateNoFlyZonesRequest { zones });

        let response = client.update_no_fly_zones(request).await?;

        println!("RESPONSE={:?}", response.into_inner());
    }

    // Best Path During Temporary No-Fly Zone
    {
        println!("\n\u{26D4}\u{1F681} Best Path DURING Temporary No-Fly Zone");
        let request = tonic::Request::new(BestPathRequest {
            node_uuid_start: VERTIPORT_1_UUID.to_string(),
            node_uuid_end: VERTIPORT_2_UUID.to_string(),
            time_start: datetime_to_timestamp(&no_fly_start_time),
            time_end: datetime_to_timestamp(&(no_fly_start_time + Duration::hours(1))),
        });

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
        let request = tonic::Request::new(BestPathRequest {
            node_uuid_start: VERTIPORT_1_UUID.to_string(),
            node_uuid_end: VERTIPORT_2_UUID.to_string(),
            time_start: datetime_to_timestamp(&(no_fly_end_time + Duration::seconds(1))),
            time_end: datetime_to_timestamp(&(no_fly_end_time + Duration::hours(1))),
        });

        let response = client.best_path(request).await?.into_inner();

        println!("RESPONSE={:?}", response);
        println!(
            "\x1b[33;3m{} segment(s) in path.\x1b[0m",
            response.segments.len()
        );
    }

    {
        println!("\n\u{1F44D} Ready Check");
        let response = client
            .is_ready(tonic::Request::new(ReadyRequest {}))
            .await?
            .into_inner();

        println!("RESPONSE={:?}", response);
    }

    Ok(())
}
