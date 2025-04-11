use tokio::{io, net::TcpStream};
use tokio::io::AsyncWriteExt;
use tokio_util::bytes::Bytes;
use bollard::{Docker, image::BuildImageOptions};
use futures_util::{Stream, StreamExt, TryStreamExt};
use std::{error::Error, path::Path, pin::Pin, io::Cursor};
use tar::Builder;
use tokio_util::codec::{FramedRead, BytesCodec};
use kube::{Api, Client};
use k8s_openapi::api::{apps::v1::Deployment, core::v1::Pod};
use kube::api::{ListParams, PostParams};
use std::fs;
use http_body_util::{Either, Full};

fn map_hyper_error(e: std::io::Error) -> std::io::Error {
    std::io::Error::new(std::io::ErrorKind::Other, e.to_string())
}

#[derive(Debug)]
struct Frame<T> {
    data: T,
}

impl<T> Frame<T> {
    fn new(data: T) -> Self {
        Frame { data }
    }
}

async fn build_docker_image() -> Result<(), Box<dyn Error>> {
    let docker = Docker::connect_with_local_defaults()?;
    let context_path = Path::new("/usr/src/app/src");

    let mut archive = Vec::new();
    {
        let mut tar = Builder::new(&mut archive);
        tar.append_dir_all(".", context_path)?;
        tar.finish()?;
    }

    let cursor = Cursor::new(archive);
    let stream = FramedRead::new(cursor, BytesCodec::new())
        .map_ok(|bytes| bytes.freeze())
        .map_err(|e| map_hyper_error(e));

    let boxed_stream: Pin<Box<dyn Stream<Item = Result<Bytes, std::io::Error>> + Send>> =
        Box::pin(stream);

    let frame_stream = boxed_stream.map(|item| {
        item.map(|bytes| Frame::new(bytes))
    });

    let mut full_bytes = Vec::new();
    let frame_stream = frame_stream.for_each(|item| {
        if let Ok(frame) = item {
            full_bytes.extend_from_slice(&frame.data);
        }
        std::future::ready(())
    }).await;

    let full_bytes = Bytes::from(full_bytes);
    let body = Either::Left(Full::from(full_bytes));

    let options = BuildImageOptions {
        dockerfile: "Dockerfile",
        t: "gameserver:latest",
        rm: true,
        ..Default::default()
    };

    let mut build_stream = docker.build_image(options, None, Some(body));

    while let Some(update) = build_stream.next().await {
        match update {
            Ok(update) => {
                if let Some(msg) = update.stream {
                    print!("{}", msg);
                }
                if let Some(err) = update.error {
                    return Err(err.into());
                }
            }
            Err(e) => return Err(e.into()),
        }
    }

    println!("\nImage built successfully!");
    Ok(())
}



async fn create_k8s_deployment(client: &Client) -> Result<(), Box<dyn Error>> {
    let deployment_yaml = fs::read_to_string("/usr/src/app/src/gameserver/deployment.yaml")?;

    for doc in deployment_yaml.split("---") {
        let trimmed = doc.trim();
        if trimmed.is_empty() {
            continue;
        }

        if let Ok(deployment) = serde_yaml::from_str::<Deployment>(trimmed) {
            let api: Api<Deployment> = Api::namespaced(client.clone(), "default");
            api.create(&PostParams::default(), &deployment).await?;
            println!("Deployment created successfully!");
        } else if let Ok(service) = serde_yaml::from_str::<Service>(trimmed) {
            let api: Api<Service> = Api::namespaced(client.clone(), "default");
            api.create(&PostParams::default(), &service).await?;
            println!("Service created successfully!");
        } else {
            eprintln!("Unknown or unsupported YAML document:\n{}", trimmed);
        }
    }

    Ok(())
}

#[tokio::main]
async fn connect_to_server() -> Result<(), io::Error> {
    let mut stream = TcpStream::connect("gameserver-service:8080").await?;
    stream.write(b"hello world!").await?;
    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    build_docker_image().await?;

    let client = Client::try_default().await?;

    create_k8s_deployment(&client).await?;
    connect_to_server();
    let pods: Api<Pod> = Api::all(client.clone());
    let lp = ListParams::default();
    let pod_list = pods.list(&lp).await?;

    for p in pod_list.items {
        if let Some(name) = p.metadata.name {
            println!("Pod: {}", name);
        }
    }

    Ok(())
}
