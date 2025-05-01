use axum::{
    routing::{get, post},
    Router,
    response::{Html, IntoResponse, Json},
    http::{Method, StatusCode, Response},
    extract::State,
};
use axum::extract::Request;
use axum::routing::get_service;
// use axum::body::BoxBody;
use axum::body::{Bytes};
use axum::body::Body;
use http_body_util::BodyExt;
// use std::convert::Infallible;
// use http_body_util::combinators::BoxBody as HttpBoxBody;

// type BoxBody = HttpBoxBody<Bytes, Infallible>;

use std::{
    boxed, error::Error, fs, net::SocketAddr, path::Path, sync::Arc
};
use tokio::{
    fs as tokio_fs,
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    time::{sleep, Duration, timeout},
    sync::{mpsc::{self, Sender, Receiver}, Mutex},
};
use tower_http::{
    services::ServeDir,
    cors::{CorsLayer, Any},
};
use mime_guess::from_path;
use serde::{Serialize, Deserialize};
use serde_json::json;
use kube::Client;


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

static ENABLE_INITIAL_CONNECTION: bool = true;
static FORCE_REBUILD: bool = false;

static BUILD_DOCKER_IMAGE: bool = true;
static BUILD_DEPLOYMENT: bool = true;

async fn connect_to_server(mut rx: Receiver<Vec<u8>>) -> Result<(), Box<dyn Error>> {
    loop {
        let connection_result = timeout(Duration::from_secs(20), TcpStream::connect("gameserver-service:8080")).await;

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

#[derive(Clone)]
struct AppState {
    tx: Arc<Mutex<Sender<Vec<u8>>>>,
    base_path: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    println!("Redo");
    let base_path = std::env::var("SITE_URL")
        .map(|s| {
            let mut s = s.trim().to_string();
            if s.is_empty() {
                return s;
            }
            if !s.starts_with('/') {
                s.insert(0, '/');
            }
            if s.ends_with('/') && s != "/" {
                s.pop();
            }
            s
        })
        .unwrap_or_default();

    const ENABLE_INITIAL_CONNECTION: bool = true;
    const FORCE_REBUILD: bool = false;
    const BUILD_DOCKER_IMAGE: bool = false;
    const BUILD_DEPLOYMENT: bool = false;

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
        //
        //
        //
        println!("Building!");
        if BUILD_DOCKER_IMAGE {
            docker::build_docker_image().await?;
        }
        if BUILD_DEPLOYMENT {
            kubernetes::create_k8s_deployment(&kube::Client::try_default().await?).await?;
        }
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

    let state = AppState {
        tx: tx.clone(),
        base_path: base_path.clone(),
    };

    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods([Method::GET, Method::POST])
        .allow_headers(Any);

        let app = Router::new()
                .route("/message", get(get_message))
                .route("/api/send", post(receive_message))
                // here we pass in our Arc<AppState> so it builds the service:
                .fallback_service(routes_static(Arc::new(state.clone().into())))
                .layer(cors)
                .with_state(state.clone());

    let app = if !base_path.is_empty() {
        Router::new().nest(&base_path, app)
    } else {
        app
    };

    let addr: SocketAddr = "127.0.0.1:8080".parse().unwrap();
    let listener = TcpListener::bind(addr).await.unwrap();
    println!("Listening on http://{}", addr);

    axum::serve(listener, app).await.unwrap();

    Ok(())
}
//

// async fn index(State(state): State<AppState>) -> Html<String> {
//     let html = fs::read_to_string("src/html/index.html").await.unwrap();
//     let replaced = html.replace("{{SITE_URL}}", &state.base_path);
//     Html(replaced)
// }

async fn receive_message(
    State(state): State<AppState>,
    Json(payload): Json<IncomingMessage>,
) -> Json<ResponseMessage> {
    println!("Got message: {}", payload.message);

    let json_payload = MessagePayload {
        message: payload.message.clone(),
    };
    let json_bytes = serde_json::to_vec(&json_payload).unwrap();

    if let Err(e) = state.tx.lock().await.send(json_bytes).await {
        eprintln!("Failed to send message to TCP stream: {}", e);
    }

    Json(ResponseMessage {
        response: format!("You sent: {}", payload.message),
    })
}


async fn serve_html_with_replacement(
    file: &str,
    state: &AppState,
) -> Result<Response<Body>, StatusCode> {
    // Build the full path to your static dir + requested file
    let path = Path::new("src/html").join(file);

    // If it's an HTML file, read, replace, and render
    if path.extension().and_then(|e| e.to_str()) == Some("html") {
        let html = tokio_fs::read_to_string(&path)
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        let replaced = html.replace("{{SITE_URL}}", &state.base_path);
        // Html<String>.into_response() yields Response<BoxBody>
        return Ok(Html(replaced).into_response());
    }

    // Otherwise just stream the bytes
    let bytes = tokio_fs::read(&path)
        .await
        .map_err(|_| StatusCode::NOT_FOUND)?;
    let content_type = from_path(&path).first_or_octet_stream().to_string();

    Ok(Response::builder()
        .header("Content-Type", content_type)
        // Vec<u8> → Bytes → BoxBody
        .body(Body::from(bytes))
        .unwrap())
}


fn routes_static(state: Arc<AppState>) -> Router {
    Router::new().fallback(move |req: Request<Body>| {
        let state = state.clone();
        async move {
            let req_path = req.uri().path();
            let file = if req_path == "/" {
                "index.html"
            } else {
                &req_path[1..]
            };

            match serve_html_with_replacement(file, &state).await {
                // Ok already is Response<BoxBody>
                Ok(res) => res,

                // Err must also build a Response<BoxBody>
                Err(status) => {
                    let body = format!("Error serving `{}`", file);
                    Response::builder()
                        .status(status)
                        .header("content-type", "text/plain")
                        .body(body.into())
                        .unwrap()
                }
            }
        }
    })
}



async fn get_message() -> Json<MessagePayload> {
    let addr = "gameserver-service:8080";

    match TcpStream::connect(addr).await {
        Ok(mut stream) => {
            if let Err(e) = stream.write_all(b"get_message\n").await {
                eprintln!("Failed to write to TCP server: {}", e);
                return Json(MessagePayload {
                    message: "Failed to write to TCP server".into(),
                });
            }

            let mut buffer = [0u8; 1024];
            match stream.read(&mut buffer).await {
                Ok(n) => {
                    let response = String::from_utf8_lossy(&buffer[..n]).to_string();
                    Json(MessagePayload { message: response })
                }
                Err(e) => {
                    eprintln!("Failed to read from TCP server: {}", e);
                    Json(MessagePayload {
                        message: "Failed to read from TCP server".into(),
                    })
                }
            }
        }
        Err(e) => {
            eprintln!("Failed to connect to TCP server: {}", e);
            Json(MessagePayload {
                message: "Failed to connect to TCP server".into(),
            })
        }
    }
}

