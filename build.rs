use std::error::Error;

#[cfg(feature = "grpc_experimental")]
fn proto_compile() -> Result<(), Box<dyn Error>> {
    use std::fs;

    //.type_attribute(".main", "#[derive(serde::Serialize, serde::Deserialize)]")

    let proto_paths = [
        "experimental/proto/main.proto",
        "experimental/proto/kube.proto",
    ];

    let mut message_names: Vec<String> = Vec::new();
    for proto_path in &proto_paths {
        let proto_src = fs::read_to_string(proto_path)?;
        message_names.extend(proto_src.lines().filter_map(|line| {
            let line = line.trim();
            if let Some(rest) = line.strip_prefix("message ") {
                rest.split(|c: char| c == '{' || c.is_whitespace())
                    .next()
                    .map(|name| name.to_string())
            } else {
                None
            }
        }));
    }
    message_names.retain(|name| name != "IntegrationKeyRequest");

    let mut builder = tonic_prost_build::configure()
        .build_server(true)
        .build_client(true);

    for name in &message_names {
        builder = builder.type_attribute(
            format!("main.{name}"),
            "#[derive(serde::Serialize, serde::Deserialize)]",
        );
    }

    builder.compile_protos(&proto_paths, &["experimental/proto"])?;
    Ok(())
}
#[cfg(not(feature = "grpc_experimental"))]
fn proto_compile() -> Result<(), Box<dyn Error>> {
    Ok(())
}

fn main() -> Result<(), Box<dyn Error>> {
    proto_compile()?;

    Ok(())
}