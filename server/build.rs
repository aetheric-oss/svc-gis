//! build script to generate .rs from .proto

///generates .rs files in src directory
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let proto_dir = "../proto";
    let proto_file = &format!("{}/grpc.proto", proto_dir);

    let server_config = tonic_build::configure()
        .type_attribute("ReadyRequest", "#[derive(Eq, Copy)]")
        .type_attribute("ReadyResponse", "#[derive(Eq, Copy)]")
        .type_attribute("UpdateResponse", "#[derive(Eq, Copy)]")
        .type_attribute("PathSegment", "#[derive(Copy)]")
        .type_attribute("NodeType", "#[derive(postgres_types::FromSql)]")
        .type_attribute("NodeType", "#[derive(num_derive::FromPrimitive)]")
        .type_attribute("NodeType", r#"#[postgres(name = "nodetype")]"#)
        .type_attribute("Coordinates", "#[derive(Copy)]");

    let client_config = server_config.clone();

    client_config
        .build_server(false)
        .out_dir("../client-grpc/src/")
        .compile(&[proto_file], &[proto_dir])?;

    // Build the Server
    server_config
        .build_client(false)
        .compile(&[proto_file], &[proto_dir])?;

    println!("cargo:rerun-if-changed={}", proto_file);

    Ok(())
}
