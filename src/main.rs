use std::{
    error::Error,
    fs,
    net::SocketAddr,
    sync::Arc,
};

use tower_http::services::ServeDir;
use tower_http::cors::{CorsLayer, Any};
use axum::http::Method;

use axum::{
    extract::State,
    routing::{get, get_service, post},
    Router,
    Json,
};
use kube::Client;
use tokio::{
    io::AsyncWriteExt,
    net::{TcpListener, TcpStream},
    time::{sleep, Duration, timeout},
    sync::{mpsc::{self, Sender, Receiver}, Mutex},
};
use serde_json::json;
use serde::{Serialize, Deserialize};

mod kubernetes;
mod docker;

#[derive(Serialize)]
struct MessagePayload {
    message: String,
}

#[derive(Deserialize)]
struct IncomingMessage {
    message: String,
}

#[derive(Serialize)]
struct ResponseMessage {
    response: String,
}

static ENABLE_INITIAL_CONNECTION: bool = false;
static FORCE_REBUILD: bool = false;

async fn connect_to_server(mut rx: Receiver<Vec<u8>>) -> Result<(), Box<dyn Error>> {
    loop {
        let connection_result = timeout(Duration::from_secs(20), TcpStream::connect("10.43.25.184:8080")).await;

        match connection_result {
            Ok(Ok(mut stream)) => {
                println!("Successfully connected to server!");

                loop {
                    match rx.recv().await {
                        Some(data) => {
                            if let Err(e) = stream.write_all(&data).await {
                                eprintln!("Failed to send data: {}", e);
                                break;
                            }
                        }
                        None => {
                            println!("Channel closed. Reconnecting...");
                            break;
                        }
                    }
                }
            }
            Ok(Err(e)) => {
                println!("Failed to connect: {}. Retrying in 2 seconds...", e);
                sleep(Duration::from_secs(2)).await;
            }
            Err(_) => {
                println!("Connection attempt timed out. Retrying in 2 seconds...");
                sleep(Duration::from_secs(2)).await;
            }
        }
    }
}

async fn try_connect_to_server() -> Result<(), Box<dyn Error>> {
    println!("Entering the connection attempt loop...");
    for _ in 0..1 {
        if connect_to_server_inner().await.is_ok() {
            return Ok(());
        }
        println!("Retrying connection...");
        tokio::time::sleep(Duration::from_secs(2)).await;
    }
    Err(Box::new(std::io::Error::new(std::io::ErrorKind::TimedOut, "Failed to connect")))
}

async fn connect_to_server_inner() -> Result<(), Box<dyn Error>> {
    let client = Client::try_default().await?;
    let pods = kube::Api::<k8s_openapi::api::core::v1::Pod>::all(client.clone());
    let lp = kube::api::ListParams::default();
    let pod_list = pods.list(&lp).await?;

    for pod in pod_list.items {
        if let Some(name) = pod.metadata.name {
            println!("Pod: {}", name);
        }
    }

    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let initial_connection_result = if ENABLE_INITIAL_CONNECTION {
        println!("Trying initial connection...");
        try_connect_to_server().await
    } else {
        println!("Initial connection disabled.");
        Ok(())
    };

    if initial_connection_result.is_ok() && !FORCE_REBUILD {
        println!("Initial connection succeeded.");
    } else {
        if let Err(e) = initial_connection_result {
            eprintln!("Initial connection failed: {}, proceeding to build Docker image and deploy", e);
        }
        docker::build_docker_image().await?;
        kubernetes::create_k8s_deployment(&Client::try_default().await?).await?;
    }
   
    let (tx_raw, rx): (Sender<Vec<u8>>, Receiver<Vec<u8>>) = mpsc::channel(32);
    let tx = Arc::new(Mutex::new(tx_raw));

    tokio::spawn(async move {
        if let Err(e) = connect_to_server(rx).await {
            eprintln!("Connection task failed: {}", e);
        }
    });

    let payload = MessagePayload {
        message: "hello world!".to_string(),
    };
    let json_bytes = serde_json::to_vec(&payload)?;
    tx.lock().await.send(json_bytes).await?;
    println!("Sent JSON message through the channel");

    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods([Method::GET, Method::POST])
        .allow_headers(Any);

    let app = Router::new()
        .route("/message", get(get_message))
        .route("/api/send", post(receive_message))
        .fallback_service(routes_static())
        .layer(cors)
        .with_state(tx.clone());

    let addr: SocketAddr = "127.0.0.1:8080".parse().unwrap();
    let listener = TcpListener::bind(addr).await.unwrap();
    println!("Listening on http://{}", addr);

    axum::serve(listener, app).await.unwrap();

    Ok(())
}

async fn get_message() -> Json<serde_json::Value> {
    Json(json!({
        "message": "Hello from Axum!"
    }))
}

async fn receive_message(
    State(tx): State<Arc<Mutex<Sender<Vec<u8>>>>>,
    Json(payload): Json<IncomingMessage>,
) -> Json<ResponseMessage> {
    println!("Got message: {}", payload.message);

    let json_payload = MessagePayload {
        message: payload.message.clone(),
    };
    let json_bytes = serde_json::to_vec(&json_payload).unwrap();

    if let Err(e) = tx.lock().await.send(json_bytes).await {
        eprintln!("Failed to send message to TCP stream: {}", e);
    }

    Json(ResponseMessage {
        response: format!("You sent: {}", payload.message),
    })
}

fn routes_static() -> Router {
    Router::new().fallback_service(get_service(ServeDir::new("/usr/src/app/src/html/")).handle_error(|err| async move {
        (
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            format!("Unhandled internal error: {}", err),
        )
    }))
}
