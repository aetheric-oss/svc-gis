//! build script to generate .rs from .proto

///generates .rs files in src directory
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let proto_dir = "../proto";
    let proto_file = &format!("{}/grpc.proto", proto_dir);

    let server_config = tonic_build::configure()
        .extern_path(
            ".google.protobuf.Timestamp",
            "::lib_common::time::Timestamp",
        )
        .type_attribute("ReadyRequest", "#[derive(Eq, Copy)]")
        .type_attribute("ReadyResponse", "#[derive(Eq, Copy)]")
        .type_attribute("UpdateResponse", "#[derive(Eq, Copy)]")
        .type_attribute("CheckIntersectionResponse", "#[derive(Eq, Copy)]")
        .type_attribute(
            "PointZ",
            "#[derive(Copy, ::serde::Serialize, ::serde::Deserialize)]",
        )
        .type_attribute("PathSegment", "#[derive(Copy)]")
        .type_attribute("Coordinates", "#[derive(Copy)]");

    let client_config = server_config.clone();

    client_config
        .extern_path(".grpc.AircraftType", "crate::prelude::AircraftType")
        .extern_path(
            ".grpc.OperationalStatus",
            "crate::prelude::OperationalStatus",
        )
        .build_server(false)
        .out_dir("../client-grpc/src/")
        .compile(&[proto_file], &[proto_dir])?;

    // Build the Server
    server_config
        .type_attribute("NodeType", "#[derive(::num_derive::FromPrimitive)]")
        .type_attribute("NodeType", "#[derive(::strum::EnumString)]")
        .type_attribute("NodeType", "#[derive(::strum::Display)]")
        .type_attribute("ZoneType", "#[derive(::strum::EnumString)]")
        .type_attribute("ZoneType", "#[derive(::strum::Display)]")
        .type_attribute("ZoneType", "#[derive(::strum::EnumIter)]")
        .type_attribute("ZoneType", "#[derive(::postgres_types::FromSql)]")
        .type_attribute("ZoneType", "#[derive(::postgres_types::ToSql)]")
        .type_attribute("ZoneType", "#[derive(::num_derive::FromPrimitive)]")
        .type_attribute("ZoneType", r#"#[postgres(name = "zonetype")]"#)
        .build_client(false)
        .compile(&[proto_file], &[proto_dir])?;

    println!("cargo:rerun-if-changed={}", proto_file);

    Ok(())
}
