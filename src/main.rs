use std::{error::Error, net::SocketAddr, path::Path, sync::Arc};

use axum::{
    body::Body,
    extract::{Request, State, ws::{WebSocketUpgrade, WebSocket, Message}},
    http::{Method, Response, StatusCode},
    response::{Html, IntoResponse, Json},
    routing::{get, post},
    Router,
};
use futures_util::{sink::SinkExt, stream::StreamExt};
use kube::Client;
use mime_guess::from_path;
use serde::{Deserialize, Serialize};
use tokio::{
    fs as tokio_fs,
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    sync::{mpsc::{self, Receiver, Sender}, Mutex},
    time::{timeout, Duration},
};
use tower_http::{
    cors::{Any, CorsLayer},
};

mod docker;
mod kubernetes;

// Constants
const CONNECTION_RETRY_DELAY: Duration = Duration::from_secs(2);
const CONNECTION_TIMEOUT: Duration = Duration::from_secs(20);
const CHANNEL_BUFFER_SIZE: usize = 32;

// Message structures
#[derive(Debug, Serialize, Deserialize)]
struct MessagePayload {
    message: String,
}

#[derive(Debug, Deserialize)]
struct ConsoleMessage {
    data: String,
    #[serde(rename = "type")]
    message_type: String,
}

#[derive(Debug, Deserialize, Serialize)]
struct InnerData {
    data: String,
    #[serde(rename = "type")]
    message_type: String,
}

#[derive(Debug, Deserialize)]
struct IncomingMessage {
    message: String,
}

#[derive(Debug, Serialize)]
struct ResponseMessage {
    response: String,
}

#[derive(Debug, Serialize)]
struct List {
    list: Vec<String>,
}

#[derive(Clone)]
struct AppState {
    tx: Arc<Mutex<Sender<Vec<u8>>>>,
    rx: Arc<Mutex<Receiver<Vec<u8>>>>,
    base_path: String,
    client: Client,
    ws_tx: Arc<Mutex<Sender<String>>>,
    ws_rx: Arc<Mutex<Receiver<String>>>,
}

async fn attempt_connection() -> Result<TcpStream, Box<dyn Error + Send + Sync>> {
    timeout(CONNECTION_TIMEOUT, TcpStream::connect("gameserver-service:8080"))
        .await?
        .map_err(Into::into)
}

async fn handle_server_data(
    data: &[u8],
    ws_tx: &Sender<String>,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    if let Ok(text) = String::from_utf8(data.to_vec()) {
        println!("{}", text);
        
        match serde_json::from_str::<ConsoleMessage>(&text) {
            Ok(console_msg) => {
                if let Ok(inner_data) = serde_json::from_str::<InnerData>(&console_msg.data) {
                    // println!("Inner data parsed: {:?}", inner_data);
                    ws_tx.send(serde_json::to_string(&inner_data.data)?).await?;
                } else if console_msg.message_type == "stdout" {
                    ws_tx.send(console_msg.data).await?;
                } else if console_msg.message_type == "stderr" {
                    ws_tx.send(console_msg.data).await?;
                }
            }
            Err(e) => eprintln!("Failed to parse ConsoleMessage: {}", e),
        }
    } else {
        println!("Received non-UTF8 data from server: {:?}", data);
    }
    Ok(())
}

async fn handle_stream(
    rx: Arc<Mutex<Receiver<Vec<u8>>>>,
    stream: &mut TcpStream,
    ws_tx: Sender<String>,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    let (mut reader, mut writer) = stream.split();
    let mut buf = vec![0u8; 1024];

    loop {
        tokio::select! {
            // Reading from the server
            result = reader.read(&mut buf) => {
                match result {
                    Ok(0) => {
                        println!("Server closed the connection.");
                        break Ok(());
                    }
                    Ok(n) => handle_server_data(&buf[..n], &ws_tx).await?,
                    Err(e) => return Err(e.into()),
                }
            }

            // Sending data to the server
            maybe_data = async {
                rx.lock().await.recv().await
            } => {
                if let Some(data) = maybe_data {
                    writer.write_all(&data).await?;
                } else {
                    println!("Channel closed. Reconnecting...");
                    break Ok(());
                }
            }
        }
    }
}

async fn connect_to_server(
    rx: Arc<Mutex<Receiver<Vec<u8>>>>,
    ws_tx: Sender<String>,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    loop {
        match attempt_connection().await {
            Ok(mut stream) => {
                println!("Successfully connected to server!");
                handle_stream(rx.clone(), &mut stream, ws_tx.clone()).await?;
            }
            Err(e) => {
                eprintln!("Failed to connect: {}. Retrying...", e);
                tokio::time::sleep(CONNECTION_RETRY_DELAY).await;
            }
        }
    }
}

async fn try_initial_connection(
    ws_tx: Sender<String>,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    match attempt_connection().await {
        Ok(mut stream) => {
            println!("Initial connection succeeded!");
            let rx = Arc::new(Mutex::new(tokio::sync::mpsc::channel(32).1));
            handle_stream(rx, &mut stream, ws_tx).await
        }
        Err(e) => {
            eprintln!("Initial connection failed: {}", e);
            Err(e)
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error + Send + Sync>> {
    println!("Starting server...");

    // Configuration
    let base_path = std::env::var("SITE_URL")
        .map(|s| {
            let mut s = s.trim().to_string();
            if !s.is_empty() {
                if !s.starts_with('/') {
                    s.insert(0, '/');
                }
                if s.ends_with('/') && s != "/" {
                    s.pop();
                }
            }
            s
        })
        .unwrap_or_default();

    const ENABLE_INITIAL_CONNECTION: bool = true;
    const FORCE_REBUILD: bool = false;
    const BUILD_DOCKER_IMAGE: bool = true;
    const BUILD_DEPLOYMENT: bool = true;

    // Initialize channels and Kubernetes client
    let (ws_tx, ws_rx) = mpsc::channel::<String>(CHANNEL_BUFFER_SIZE);
    let (tx, rx) = mpsc::channel::<Vec<u8>>(CHANNEL_BUFFER_SIZE);
    let client = Client::try_default().await?;

    // Initial connection attempt
    if ENABLE_INITIAL_CONNECTION {
        println!("Trying initial connection...");
        if try_initial_connection(ws_tx.clone()).await.is_err() || FORCE_REBUILD {
            eprintln!("Initial connection failed or force rebuild enabled");
            if BUILD_DOCKER_IMAGE {
                docker::build_docker_image().await?;
            }
            if BUILD_DEPLOYMENT {
                kubernetes::create_k8s_deployment(&client).await?;
            }
        }
    }

    // Start persistent connection task
    let rx_arc = Arc::new(Mutex::new(rx));
    let rx_arc_clone = Arc::clone(&rx_arc); // <- clone before moving into task
    let ws_tx_clone = ws_tx.clone();
    
    tokio::spawn(async move {
        if let Err(e) = connect_to_server(rx_arc_clone, ws_tx_clone).await {
            eprintln!("Connection task failed: {}", e);
        }
    });
    
    let state = AppState {
        tx: Arc::new(Mutex::new(tx)),
        rx: rx_arc, // original one is still here
        base_path: base_path.clone(),
        client,
        ws_tx: Arc::new(Mutex::new(ws_tx)),
        ws_rx: Arc::new(Mutex::new(ws_rx)),
    };
    

    // Configure CORS
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods([Method::GET, Method::POST])
        .allow_headers(Any);

    // Set up routes
    let inner_app = Router::new()
    .route("/message", get(get_message))
    .route("/api/send", post(receive_message))
    .route("/api/general", post(process_general))
    .route("/api/nodes", get(get_nodes))
    .route("/ws", get(ws_handler))
    .fallback_service(routes_static(Arc::new(state.clone())))
    .with_state(state);

    let app = if base_path.is_empty() || base_path == "/" {
        inner_app.layer(cors)
    } else {
        Router::new()
            .nest(&base_path, inner_app)
            .layer(cors)
    };

    // Start server
    let addr: SocketAddr = "127.0.0.1:8080".parse().unwrap();
    println!("Listening on http://{}", addr);
    axum::serve(TcpListener::bind(addr).await?, app).await?;

    Ok(())
}

// WebSocket handler
async fn ws_handler(ws: WebSocketUpgrade, State(state): State<AppState>) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_socket(socket, state))
}

async fn handle_socket(socket: WebSocket, state: AppState) {
    println!("WebSocket connected");
    let (mut sender, mut receiver) = socket.split();

    // Task to forward messages from ws_rx to WebSocket
    let ws_rx = state.ws_rx.clone();
    tokio::spawn(async move {
        while let Some(message) = ws_rx.lock().await.recv().await {
            if let Err(e) = sender.send(Message::Text(message.into())).await {
                eprintln!("Error sending WebSocket message: {}", e);
                break;
            }
        }
    });

    // Handle incoming WebSocket messages
    while let Some(Ok(Message::Text(text))) = receiver.next().await {
        println!("Got from client: {}", text);
        let json_payload = MessagePayload { message: text.to_string() };
        if let Ok(json_bytes) = serde_json::to_vec(&json_payload) {
            if let Err(e) = state.tx.lock().await.send(json_bytes).await {
                eprintln!("Failed to send message to TCP stream: {}", e);
            }
        }
    }

    println!("WebSocket disconnected");
}

// API endpoints
async fn get_nodes(State(state): State<AppState>) -> impl IntoResponse {
    match kubernetes::list_node_names(state.client).await {
        Ok(nodes) => Json(List { list: nodes }),
        Err(err) => {
            eprintln!("Error listing nodes: {}", err);
            Json(List { list: vec![] })
        },
    }
}

async fn process_general(State(state): State<AppState>, Json(payload): Json<IncomingMessage>) {
    if payload.message == "create_server" {
        let json_payload = MessagePayload { message: payload.message };
        if let Ok(json_bytes) = serde_json::to_vec(&json_payload) {
            if let Err(e) = state.tx.lock().await.send(json_bytes).await {
                eprintln!("Failed to send message: {}", e);
            }
        }
    }
}

async fn receive_message(
    State(state): State<AppState>,
    Json(payload): Json<IncomingMessage>,
) -> Json<ResponseMessage> {
    let json_payload = MessagePayload { message: payload.message.clone() };
    if let Ok(json_bytes) = serde_json::to_vec(&json_payload) {
        if let Err(e) = state.tx.lock().await.send(json_bytes).await {
            eprintln!("Failed to send message: {}", e);
        }
    }

    Json(ResponseMessage {
        response: format!("You sent: {}", payload.message),
    })
}

// Static file serving
async fn serve_html_with_replacement(
    file: &str,
    state: &AppState,
) -> Result<Response<Body>, StatusCode> {
    let path = Path::new("src/html").join(file);

    if path.extension().and_then(|e| e.to_str()) == Some("html") {
        let html = tokio_fs::read_to_string(&path)
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        let replaced = html.replace("{{SITE_URL}}", &state.base_path);
        return Ok(Html(replaced).into_response());
    }

    let bytes = tokio_fs::read(&path)
        .await
        .map_err(|_| StatusCode::NOT_FOUND)?;
    let content_type = from_path(&path).first_or_octet_stream().to_string();

    Ok(Response::builder()
        .header("Content-Type", content_type)
        .body(Body::from(bytes))
        .unwrap())
}

fn routes_static(state: Arc<AppState>) -> Router {
    Router::new().fallback(move |req: Request<Body>| {
        let state = state.clone();
        async move {
            let file = if req.uri().path() == "/" {
                "index.html"
            } else {
                &req.uri().path()[1..]
            };

            match serve_html_with_replacement(file, &state).await {
                Ok(res) => res,
                Err(status) => Response::builder()
                    .status(status)
                    .header("content-type", "text/plain")
                    .body(format!("Error serving `{}`", file).into())
                    .unwrap(),
            }
        }
    })
}

async fn get_message(State(state): State<AppState>) -> Json<MessagePayload> {
    let request = MessagePayload {
        message: "get_message".to_string(),
    };

    let json_bytes = match serde_json::to_vec(&request) {
        Ok(bytes) => bytes,
        Err(e) => {
            eprintln!("Serialization error: {}", e);
            return Json(MessagePayload {
                message: "Serialization failed".to_string(),
            });
        }
    };

    if let Err(e) = state.tx.lock().await.send(json_bytes).await {
        eprintln!("Failed to send message: {}", e);
    }

    match state.rx.lock().await.recv().await {
        Some(response_bytes) => match serde_json::from_slice::<MessagePayload>(&response_bytes) {
            Ok(msg) => Json(msg),
            Err(e) => {
                eprintln!("Deserialization error: {}", e);
                Json(MessagePayload {
                    message: "Deserialization failed".to_string(),
                })
            }
        },
        None => {
            eprintln!("No response received");
            Json(MessagePayload {
                message: "No response received".to_string(),
            })
        }
    }
}