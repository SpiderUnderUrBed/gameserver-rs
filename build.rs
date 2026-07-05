use std::error::Error;


#[cfg(feature = "grpc_experimental")]
fn proto_compile() -> Result<(), Box<dyn Error>> {
    println!("cargo:warning=OUT_DIR is {}", std::env::var("OUT_DIR").unwrap());
    tonic_prost_build::compile_protos("experimental/proto/main.proto")?;
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