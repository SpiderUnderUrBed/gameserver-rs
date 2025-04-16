use std::{
    error::Error,
    fs::{self, File},
    io::{self, Cursor},
    net::SocketAddr,
    path::Path,
    pin::Pin,
};
use tower_http::services::ServeDir;
use walkdir::WalkDir;

use axum::{
    extract::{Path as RoutingPath, Query},
    response::IntoResponse,
    routing::{get, get_service},
    Router,
};

use bollard::image::PushImageOptions;
use bollard::{image::{BuildImageOptions, TagImageOptions}, Docker};
// use bollard::models::ErrorResponse;
// use bollard::image::RegistryAuth;
use bollard::auth::DockerCredentials;


use futures_util::{Stream, StreamExt, TryStreamExt};

use http_body_util::{Either, Full};

use k8s_openapi::api::{
    apps::v1::Deployment,
    core::v1::{Pod, Service},
};

use kube::Error::Api as ErrorApi;
use kube::Api;
// use kube::Error::Api;
use kube::{
    api::{ListParams, PostParams},
    core::params,
    Client,
};

use tar::{Builder, Header};

use tokio::{
    io::AsyncWriteExt,
    net::{TcpListener, TcpStream},
    time::{sleep, Duration},
    sync::mpsc::{self, Sender, Receiver},
};

use tokio_util::{
    bytes, codec::{BytesCodec, FramedRead}
};

use http_body_util::combinators::BoxBody;
use http_body_util::{StreamBody};

use bytes::Bytes as BytesRaw; 
use axum::body::Bytes;
use http_body::Frame; 

use serde::Serialize;

const ENABLE_TAG_AND_PUSH: bool = true; // Set to false to disable tag & push

#[derive(Debug)]
struct DockerBuildError(String);

impl std::fmt::Display for DockerBuildError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Docker build error: {}", self.0)
    }
}

impl std::error::Error for DockerBuildError {}


async fn build_docker_image() -> Result<(), Box<dyn Error>> {
    let docker = Docker::connect_with_local_defaults()?;
    let context_path = Path::new(".");
    // 1. Create in-memory tar archive with strict filtering
    let mut archive = Vec::new();
    {
        let mut tar = Builder::new(&mut archive);

        for entry in WalkDir::new(context_path)
        .min_depth(1)  // Skip the root directory itself
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let path = entry.path();
    
        // Skip non-file/non-directory and symlink entries
        if !path.is_file() && !path.is_dir() {
            continue;
        }
        if path.is_symlink() {
            continue;
        }
    
        let relative_path = path.strip_prefix(context_path)?;
        // Check if the first component is "target"
        if let Some(first_component) = relative_path.components().next() {
            if first_component.as_os_str() == "target" {
                continue; // Skip the top-level "target" directory and its contents
            }
        }
        if relative_path.starts_with("src/gameserver/target") {
            continue;
        }
    
        // Optionally skip hidden files/directories if needed:
        if let Some(relative_str) = relative_path.to_str() {
            if relative_str.starts_with('.') || relative_str.contains("/.") {
                continue;
            }
        }
    
        // Append to tar archive
        if path.is_dir() {
            // Create directory header
            let mut header = Header::new_gnu();
            header.set_size(0);
            header.set_entry_type(tar::EntryType::Directory);
            header.set_path(relative_path)?;
            header.set_cksum();
            tar.append(&header, &mut io::empty())?;
        } else {
            // Append file
            let mut file = File::open(path)?;
            let mut header = Header::new_gnu();
            header.set_path(relative_path)?;
            header.set_size(file.metadata()?.len());
            header.set_mode(0o644);
            header.set_cksum();
            tar.append(&header, &mut file)?;
        }
    }
    

        tar.finish()?;
    } // here, `tar` is dropped and the mutable borrow of `archive` is over

    // 2. Create properly typed stream
    // Wrap the `archive` in a Cursor
    let cursor = Cursor::new(archive);
    let framed: FramedRead<Cursor<Vec<u8>>, BytesCodec> = FramedRead::new(cursor, BytesCodec::new());
    
    // The BytesCodec produces BytesMut values; convert them to the Bytes type (which is really the same underlying type)
    let stream_converted = framed
        .map_ok(|bytes_mut| {
            // Convert BytesMut to axum::body::Bytes.
            // Depending on your version, you may either call `freeze` and then use it,
            // or if they are actually the same type, the conversion should be a no-op.
            let b: BytesRaw = bytes_mut.freeze();
            // axum::body::Bytes is re-exported from bytes::Bytes, so this should work:
            Bytes::from(b)
        })
        .map_ok(|b: Bytes| {
            // Now create an HTTP frame carrying that bytes value.
            Frame::data(b)
        })
        .map_err(|e| {
            std::io::Error::new(std::io::ErrorKind::Other, e)
        });

    // Box the stream into a trait object with the desired type.
    let boxed_stream: Pin<Box<dyn Stream<Item = Result<Frame<Bytes>, io::Error>> + Send>> =
        Box::pin(stream_converted);

    let stream_body = StreamBody::new(boxed_stream);

    // 3. Configure build options
    let options = BuildImageOptions {
        dockerfile: "src/gameserver/Dockerfile",
        t: "gameserver:latest",
        rm: true,
        forcerm: true,
        ..Default::default()
    };

    // 4. Execute build
    let mut build_stream = docker.build_image(
        options,
        None,
        Some(Either::Right(stream_body)),
    );

    // 5. Process build output
    while let Some(update) = build_stream.next().await {
        match update {
            Ok(update) => {
                if let Some(msg) = update.stream {
                    print!("{}", msg);
                }
                if let Some(err) = update.error {
                    return Err(DockerBuildError(err).into());
                }
            }
            Err(e) => {
                return Err(DockerBuildError(format!("Docker API error: {}", e)).into());
            }
        }
    }

    if ENABLE_TAG_AND_PUSH {
        // Tag the image before pushing
        docker.tag_image(
            "gameserver:latest",
            Some(TagImageOptions {
                repo: "localhost:5000/gameserver".to_string(),
                tag: "latest".to_string(),
            }),
        ).await?;

        // Now push the tagged image
        docker.push_image(
            "localhost:5000/gameserver",
            None::<PushImageOptions<String>>,
            None::<DockerCredentials>,
        )
        .try_collect::<Vec<_>>()
        .await?;
    }


    println!("\n✅ Docker image built successfully!");
    Ok(())
}

async fn create_k8s_deployment(client: &Client) -> Result<(), Box<dyn Error>> {
    //println!("pre");
    let deployment_yaml = fs::read_to_string("/usr/src/app/src/gameserver/deployment.yaml")?;
    //println!("post");
    // let deployment_yaml = fs::read_to_string("/home/spiderunderurbed/projects/gameserver-rs/src/gameserver/deployment.yaml")?;

    for doc in deployment_yaml.split("---") {
        let trimmed = doc.trim();
        if trimmed.is_empty() {
            continue;
        }

        if let Ok(deployment) = serde_yaml::from_str::<Deployment>(trimmed) {
            let api: Api<Deployment> = Api::namespaced(client.clone(), "default");
            match api.create(&PostParams::default(), &deployment).await {
                Ok(_) => println!("Deployment created successfully!"),
                Err(ErrorApi(err)) if err.code == 409 => {
                    println!("Deployment already exists, skipping...");
                }
                Err(e) => return Err(Box::new(e)),
            }
        } else if let Ok(service) = serde_yaml::from_str::<Service>(trimmed) {
            let api: Api<Service> = Api::namespaced(client.clone(), "default");
            match api.create(&PostParams::default(), &service).await {
                Ok(_) => println!("Service created successfully!"),
                Err(ErrorApi(err)) if err.code == 409 => {
                    println!("Service already exists, skipping...");
                }
                Err(e) => return Err(Box::new(e)),
            }
        } else {
            eprintln!("Unknown or unsupported YAML document:\n{}", trimmed);
        }
    }

    Ok(())
}

#[derive(Serialize)]
struct MessagePayload {
    message: String,
}

async fn connect_to_server(mut rx: Receiver<Vec<u8>>) -> Result<(), Box<dyn Error>> {
    loop {
        match TcpStream::connect("gameserver-service:8080").await {
            Ok(mut stream) => {
                println!("Connected to server!");

                while let Some(data) = rx.recv().await {
                    if let Err(e) = stream.write_all(&data).await {
                        eprintln!("Failed to send data: {}", e);
                        break; 
                    }
                }

                break;
            }
            Err(e) => {
                println!("Failed to connect: {}. Retrying in 2 seconds...", e);
                sleep(Duration::from_secs(2)).await;
            }
        }
    }

    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    build_docker_image().await?;
    let client = Client::try_default().await?;

    println!("Done with docker image");
    create_k8s_deployment(&client).await?;
    println!("Done with deployment");

    let (tx, rx): (Sender<Vec<u8>>, Receiver<Vec<u8>>) = mpsc::channel(32);

    tokio::spawn(async move {
        if let Err(e) = connect_to_server(rx).await {
            eprintln!("Connection task failed: {}", e);
        }
    });
    let payload = MessagePayload {
        message: "hello world!".to_string(),
    };
    let json_bytes = serde_json::to_vec(&payload)?;

    tx.send(json_bytes).await?;
    println!("Sent JSON message through the channel");

    // let message = String::from("This is a message from main");
    // tx.send(message.into_bytes()).await?;

    // println!("Sent a message through the channel");

    let app = Router::new().fallback_service(routes_static());

    let addr: SocketAddr = "127.0.0.1:8080".parse().unwrap();
    let listener = TcpListener::bind(addr).await.unwrap();
    println!("Listening on http://{}", addr);

    axum::serve(listener, app).await.unwrap();

    Ok(())
}

fn routes_static() -> Router {
    Router::new().fallback_service(get_service(ServeDir::new("/usr/src/app/src/html/")).handle_error(|err| async move {
        (
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            format!("Unhandled internal error: {}", err),
        )
    }))
}


// fn routes() -> Router {
//     Router::new()
//         .route("/hello", get(handler))
// }

// async fn handler(Path(path): Path<String>) -> impl IntoResponse {
//     Html(format!("Hello, you are at <strong>{path}</strong>"))
// }

// let pods: Api<Pod> = Api::all(client.clone());
// let lp = ListParams::default();
// let pod_list = pods.list(&lp).await?;

// for p in pod_list.items {
//     if let Some(name) = p.metadata.name {
//         println!("Pod: {}", name);
//     }
// }
