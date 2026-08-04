use std::error::Error;

#[cfg(feature = "grpc_experimental")]
fn proto_compile() -> Result<(), Box<dyn Error>> {
    use std::fs;

    //.type_attribute(".main", "#[derive(serde::Serialize, serde::Deserialize)]")

    let proto_path = "experimental/proto/main.proto";
    let proto_src = fs::read_to_string(proto_path)?;

    let message_names: Vec<String> = proto_src
        .lines()
        .filter_map(|line| {
            let line = line.trim();
            if let Some(rest) = line.strip_prefix("message ") {
                rest.split(|c: char| c == '{' || c.is_whitespace())
                    .next()
                    .map(|name| name.to_string())
            } else {
                None
            }
        })
        .filter(|name| name != "IntegrationKeyRequest")
        .collect();

    let mut builder = tonic_prost_build::configure()
        .build_server(true)
        .build_client(true);

    for name in &message_names {
        builder = builder.type_attribute(
            format!("main.{name}"),
            "#[derive(serde::Serialize, serde::Deserialize)]",
        );
    }

    builder.compile_protos(&[proto_path], &["experimental/proto"])?;
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
