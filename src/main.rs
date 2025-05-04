use std::{
    error::Error,
    fs,
    net::SocketAddr,
    path::Path,
    sync::Arc,
};

use axum::{
    body::{Body, Bytes},
    extract::{Request, State, ws::{WebSocketUpgrade, WebSocket, Message, Utf8Bytes}},
    http::{Method, Response, StatusCode},
    response::{Html, IntoResponse, Json},
    routing::{get, post, get_service},
    Router,
};

use futures_util::{sink::SinkExt, stream::StreamExt};
use http_body_util::BodyExt;
use kube::Client;
use mime_guess::from_path;
use serde::{Deserialize, Serialize};
use serde_json::json;
use tokio::{
    fs as tokio_fs,
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    sync::{mpsc::{self, Receiver, Sender}, Mutex},
    time::{sleep, timeout, Duration},
};
// use tokio_tungstenite::{accept_async, tungstenite::protocol::Message};
use tower_http::{
    cors::{Any, CorsLayer},
    services::ServeDir,
};

mod docker;
mod kubernetes;

#[derive(Serialize, Deserialize)]
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

#[derive(Serialize)]
struct List {
    list: Vec<String>,
}

// static ENABLE_INITIAL_CONNECTION: bool = true;
// static FORCE_REBUILD: bool = false;

// static BUILD_DOCKER_IMAGE: bool = true;
// static BUILD_DEPLOYMENT: bool = true;

pub async fn connect_to_server(rx: Arc<Mutex<Receiver<Vec<u8>>>>) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    loop {
        let connection_result = attempt_connection().await;

        match connection_result {
            Ok(mut stream) => {
                println!("Successfully connected to server!");
                handle_stream(rx.clone(), &mut stream).await?;
            }
            Err(e) => {
                eprintln!("Failed to connect: {}. Retrying in 2 seconds...", e);
                tokio::time::sleep(Duration::from_secs(2)).await;
            }
        }
    }
}

async fn attempt_connection() -> Result<TcpStream, Box<dyn std::error::Error + Send + Sync>> {
    let connection_result = timeout(Duration::from_secs(20), TcpStream::connect("gameserver-service:8080")).await;

    match connection_result {
        Ok(Ok(stream)) => Ok(stream),
        Ok(Err(e)) => Err(Box::new(e)),
        Err(_) => Err(Box::new(std::io::Error::new(std::io::ErrorKind::TimedOut, "Connection attempt timed out"))),
    }
}

async fn handle_stream(rx: Arc<Mutex<Receiver<Vec<u8>>>>, stream: &mut TcpStream) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    loop {
        let maybe_data = {
            let mut locked = rx.lock().await;
            locked.recv().await
        };

        match maybe_data {
            Some(data) => {
                if let Err(e) = stream.write_all(&data).await {
                    eprintln!("Failed to send data: {}", e);
                    break Ok(());
                }
            }
            None => {
                println!("Channel closed. Reconnecting...");
                break Ok(());
            }
        }
    }
}

async fn try_connect_to_server() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    println!("Entering the connection attempt loop...");
    for _ in 0..1 {
        match attempt_connection().await {
            Ok(mut stream) => {
                println!("Successfully connected to server!");
                let rx = Arc::new(Mutex::new(tokio::sync::mpsc::channel(32).1));  // example receiver
                return handle_stream(rx, &mut stream).await;
            }
            Err(_) => {
                println!("Retrying connection...");
                tokio::time::sleep(Duration::from_secs(2)).await;
            }
        }
    }
    Err(Box::new(std::io::Error::new(std::io::ErrorKind::TimedOut, "Failed to connect")))
}

#[derive(Clone)]
struct AppState {
    tx: Arc<Mutex<Sender<Vec<u8>>>>,
    rx: Arc<Mutex<Receiver<Vec<u8>>>>,
    base_path: String,
    client: Client
}


#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
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
    const BUILD_DOCKER_IMAGE: bool = true;
    const BUILD_DEPLOYMENT: bool = true;

    let initial_connection_result = if ENABLE_INITIAL_CONNECTION {
        println!("Trying initial connection...");
        try_connect_to_server().await
    } else {
        println!("Initial connection disabled.");
        Ok(())
    };

    let client = kube::Client::try_default().await?;
    if initial_connection_result.is_ok() && !FORCE_REBUILD {
        println!("Initial connection succeeded.");
    } else {
        if let Err(e) = initial_connection_result {
            eprintln!("Initial connection failed: {}, proceeding to build Docker image and deploy", e);
        }
        println!("Building!");
        if BUILD_DOCKER_IMAGE {
            docker::build_docker_image().await?;
        }
        if BUILD_DEPLOYMENT {
            kubernetes::create_k8s_deployment(&client).await?;
        }
    }

    let (tx_raw, rx_raw): (mpsc::Sender<Vec<u8>>, mpsc::Receiver<Vec<u8>>) = mpsc::channel(32);
    let tx = Arc::new(Mutex::new(tx_raw));
    let rx = Arc::new(Mutex::new(rx_raw));

    {
        let rx = Arc::clone(&rx);
        tokio::spawn(async move {
            if let Err(e) = connect_to_server(rx).await {
                eprintln!("Connection task failed: {}", e);
            }
        });
    }

    let payload = MessagePayload {
        message: "hello world!".to_string(),
    };
    let json_bytes = serde_json::to_vec(&payload)?;
    tx.lock().await.send(json_bytes).await?;
    println!("Sent JSON message through the channel");

    let state = AppState {
        tx: Arc::clone(&tx),
        rx: Arc::clone(&rx),
        base_path: base_path.clone(),
        client,
    };

    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods([Method::GET, Method::POST])
        .allow_headers(Any);

    let app = Router::new()
        .route("/message", get(get_message))
        .route("/api/send", post(receive_message))
        .route("/api/general", post(process_general))
        .route("/api/nodes", get(get_nodes))
        .route("/ws", get(ws_handler))
        .fallback_service(routes_static(Arc::new(state.clone().into())))
        .layer(cors)
        .with_state(state.clone());

    let app = if !base_path.is_empty() {
        Router::new().nest(&base_path, app)
    } else {
        app
    };

    let addr: SocketAddr = "127.0.0.1:8080".parse().unwrap();
    let listener = TcpListener::bind(addr).await?;
    println!("Listening on http://{}", addr);

    axum::serve(listener, app).await?;

    Ok(())
}

async fn ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_socket(socket, state))
}


async fn handle_socket(mut socket: WebSocket, state: AppState) {
    println!("WebSocket connected");
    
    if let Err(err) = socket.send(Message::Text(Utf8Bytes::from("Test"))).await {
        eprintln!("Failed to send message: {}", err);
    }

    while let Some(Ok(msg)) = socket.recv().await {
        if let Message::Text(text) = msg {
            println!("Got: {:?}", text);

            // You can now use `state.tx` or `state.client` here as needed
            if socket.send(Message::Text(text)).await.is_err() {
                break;
            }
        }
    }

    println!("WebSocket disconnected");
}

//

// async fn index(State(state): State<AppState>) -> Html<String> {
//     let html = fs::read_to_string("src/html/index.html").await.unwrap();
//     let replaced = html.replace("{{SITE_URL}}", &state.base_path);
//     Html(replaced)
// }

pub async fn get_nodes(State(state): State<AppState>) -> impl IntoResponse {
    println!("Sorting nodes");
    match kubernetes::list_node_names(state.client).await {
        Ok(nodes) => {
            println!("{}", nodes.join(" "));
            Json(List { list: nodes })
        }, 
        Err(err) => {
            eprintln!("Error listing nodes: {}", err);
            Json(List { list: vec![] })
        },
    }
}
//
//
async fn process_general(
    State(state): State<AppState>,
    Json(payload): Json<IncomingMessage>,
){
    println!("{}", payload.message);
    match payload.message.as_str() {
        "create_server" => {
            let json_payload = MessagePayload {
                message: payload.message.clone(),
            };
            let json_bytes = serde_json::to_vec(&json_payload).unwrap();
        
            if let Err(e) = state.tx.lock().await.send(json_bytes).await {
                eprintln!("Failed to send message to TCP stream: {}", e);
            }
        },
        _ => {}
    }
}
//
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


async fn get_message(State(state): State<AppState>) -> Json<MessagePayload> {
    // Create a request message
    let request = MessagePayload {
        message: "get_message".to_string(),
    };

    // Serialize the message to JSON bytes
    let json_bytes = match serde_json::to_vec(&request) {
        Ok(bytes) => bytes,
        Err(e) => {
            eprintln!("Serialization error: {}", e);
            return Json(MessagePayload {
                message: "Serialization failed".to_string(),
            });
        }
    };

    // Send the request
    if let Err(e) = state.tx.lock().await.send(json_bytes).await {
        eprintln!("Failed to send message through channel: {}", e);
        return Json(MessagePayload {
            message: "Failed to send request".to_string(),
        });
    }

    // Receive the response
    match state.rx.lock().await.recv().await {
        Some(response_bytes) => {
            match serde_json::from_slice::<MessagePayload>(&response_bytes) {
                Ok(msg) => Json(msg),
                Err(e) => {
                    eprintln!("Deserialization error: {}", e);
                    Json(MessagePayload {
                        message: "Deserialization failed".to_string(),
                    })
                }
            }
        }
        None => {
            eprintln!("No response received");
            Json(MessagePayload {
                message: "No response received".to_string(),
            })
        }
    }
}
