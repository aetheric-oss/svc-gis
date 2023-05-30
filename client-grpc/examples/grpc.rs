//! gRPC client implementation

use std::env;
#[allow(unused_qualifications, missing_docs)]
use svc_gis_client_grpc::client::{
    rpc_service_client::RpcServiceClient, Node, NodeType, ReadyRequest, UpdateNodesRequest,
};

/// Provide endpoint url to use
pub fn get_grpc_endpoint() -> String {
    //parse socket address from env variable or take default value
    let address = match env::var("SERVER_HOSTNAME") {
        Ok(val) => val,
        Err(_) => "localhost".to_string(), // default value
    };

    let port = match env::var("SERVER_PORT_GRPC") {
        Ok(val) => val,
        Err(_) => "50051".to_string(), // default value
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
    println!("Client created");

    // Update Nodes
    {
        let request = tonic::Request::new(UpdateNodesRequest {
            nodes: vec![
                Node {
                    uuid: "00000000-0000-0000-0000-000000000000".to_string(),
                    latitude: 1.0,
                    longitude: 1.0,
                    node_type: NodeType::Vertiport as i32,
                },
                Node {
                    uuid: "00000000-0000-0000-0000-000000000001".to_string(),
                    latitude: 2.0,
                    longitude: 2.0,
                    node_type: NodeType::Waypoint as i32,
                },
            ],
        });

        let response = client.update_nodes(request).await?;

        println!("RESPONSE={:?}", response.into_inner());
    }

    {
        let response = client
            .is_ready(tonic::Request::new(ReadyRequest {}))
            .await?;

        println!("RESPONSE={:?}", response.into_inner());
    }

    Ok(())
}
