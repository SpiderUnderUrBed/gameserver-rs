use std::collections::HashMap;
use std::{net::SocketAddr, path::Path, sync::Arc};

use axum_login::{AuthUser, AuthnBackend, UserId};
use axum::middleware::{self, Next};
use axum::{
    body::Body,
    extract::{ws::{Message, WebSocket, WebSocketUpgrade}, Request, State},
    http::{self, Method, Response, StatusCode},
    response::{Html, IntoResponse, Json},
    routing::{get, post},
    Router,
};
use futures_util::{sink::SinkExt, stream::StreamExt};
// use k8s_openapi::chrono;
use kube::Client;
use mime_guess::from_path;
use serde::{Deserialize, Serialize};
use serde_json::json;
use tokio::{
    fs as tokio_fs,
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    sync::{broadcast, mpsc, Mutex},
    time::{timeout, Duration},
};
use tower_http::cors::{Any, CorsLayer};
use bcrypt::{hash, verify, DEFAULT_COST};
use chrono::{Utc, Duration as OtherDuration};
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, TokenData, Validation};

mod docker;
mod kubernetes;

const CONNECTION_RETRY_DELAY: Duration = Duration::from_secs(2);
const CONNECTION_TIMEOUT: Duration = Duration::from_secs(3);
const CHANNEL_BUFFER_SIZE: usize = 32;

#[derive(Debug, Serialize, Deserialize)]
struct MessagePayload {
    r#type: String,
    message: String,
    authcode: String,
}

#[derive(Debug, Deserialize)]
struct ConsoleMessage {
    data: String,
    #[serde(rename = "type")]
    message_type: String,
    authcode: String,
}

#[derive(Debug, Deserialize, Serialize)]
struct InnerData {
    data: String,
    #[serde(rename = "type")]
    message_type: String,
    authcode: String,
}

#[derive(Debug, Serialize)]
struct ResponseMessage {
    response: String,
}

#[derive(Debug, Serialize)]
struct List {
    list: Vec<String>,
}

enum WebErrors {
    AuthError {
        message: String,
        status_code: StatusCode,
    }
}
impl IntoResponse for WebErrors {
    fn into_response(self) -> Response<Body> {
        let (status_code, message) = match self {
            WebErrors::AuthError { message, status_code } => (status_code, message),
        };

        Response::builder()
            .status(status_code)
            .header("content-type", "application/json")
            .body(Body::from(serde_json::to_string(&json!({ "error": message })).unwrap()))
            .unwrap()
    }
}


// impl IntoResponse for WebErrors {
//     fn into_response(self) -> Response<axum::body::Body> {
//         match self {
//             WebErrors::AuthError { message, status_code } => {
//                 Response::builder()
//                     .status(status_code)
//                     .header("content-type", "text/plain")
//                     .body(message.into())
//                     .unwrap()
//             }
//         }
//     }
// }

#[derive(Clone)]
struct AppState {
    tcp_tx: Arc<Mutex<mpsc::Sender<Vec<u8>>>>,
    tcp_rx: Arc<Mutex<mpsc::Receiver<Vec<u8>>>>,
    
    ws_tx: broadcast::Sender<String>,
    
    base_path: String,
    client: Option<Client>,
}
#[derive(Deserialize, Serialize)]
pub struct Claims {
    pub expirery: usize,
    pub iat: usize,
    pub user: String,
}

#[derive(Clone, Debug)]
pub struct User {
    pub username: String,
    pub password_hash: String
}
impl AuthUser for User {
    type Id = String;

    fn id(&self) -> Self::Id {
        self.username.clone()
    }

    fn session_auth_hash(&self) -> &[u8] {
        &self.password_hash.as_bytes()
    }
}
#[derive(Clone, Default)]
struct Backend {
    users: HashMap<i64, User>,
}

// #[async_trait]
// impl AuthnBackend for Backend {
//     type User = User;
//     type Credentials = Credentials;
//     type Error = std::convert::Infallible;

//     async fn authenticate(
//         &self,
//         Credentials { user_id }: Self::Credentials,
//     ) -> Result<Option<Self::User>, Self::Error> {
//         Ok(self.users.get(&user_id).cloned())
//     }

//     async fn get_user(
//         &self,
//         user_id: &UserId<Self>,
//     ) -> Result<Option<Self::User>, Self::Error> {
//         Ok(self.users.get(user_id).cloned())
//     }
// }

#[derive(Clone)]
struct Credentials {
    user_id: i64,
}

#[derive(Debug, Deserialize, Clone)]
struct IncomingMessage {
    message: String,
    #[serde(rename = "type")]
    message_type: String,
    authcode: String,
}
#[derive(Debug, Deserialize, Clone)]
pub struct LoginData {
    pub user: String,
    pub password: String
}
#[derive(Debug, Deserialize, Clone)]
#[serde(tag = "kind")]
enum ApiCalls {
    LoginData(LoginData),
    IncomingMessage(IncomingMessage),
}

// Trait definition (assumed)
trait ApiCall {
    fn standard_response(&self) -> Option<IncomingMessage>;
    fn login_data(&self) -> Option<LoginData>;
}

impl ApiCall for ApiCalls {
    fn standard_response(&self) -> Option<IncomingMessage> {
        match self {
            ApiCalls::IncomingMessage(data) => Some(data.clone()),
            _ => None,
        }
    }

    fn login_data(&self) -> Option<LoginData> {
        match self {
            ApiCalls::LoginData(data) => Some(data.clone()),
            _ => None,
        }
    }
}


async fn attempt_connection() -> Result<TcpStream, Box<dyn std::error::Error + Send + Sync>> {
    timeout(CONNECTION_TIMEOUT, TcpStream::connect("127.0.0.1:8082")).await?.map_err(Into::into)
}

async fn handle_server_data(
    data: &[u8],
    ws_tx: &broadcast::Sender<String>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    if let Ok(text) = String::from_utf8(data.to_vec()) {
        println!("Raw message from server: {}", text);
        
        if let Ok(outer_msg) = serde_json::from_str::<serde_json::Value>(&text) {
            if let Some(inner_data_str) = outer_msg["data"].as_str() {
                if let Ok(inner_data) = serde_json::from_str::<serde_json::Value>(inner_data_str) {
                    if let Some(message_content) = inner_data["data"].as_str() {
                        println!("Extracted message: {}", message_content);
                        let _ = ws_tx.send(message_content.to_string());
                    }
                } else {
                    println!("Sending raw inner data: {}", inner_data_str);
                    let _ = ws_tx.send(inner_data_str.to_string());
                }
            } else {
                println!("Sending raw message: {}", text);
                let _ = ws_tx.send(text);
            }
        } else {
            println!("Sending raw text: {}", text);
            let _ = ws_tx.send(text);
        }
    }
    Ok(())
}

async fn handle_stream(
    rx: Arc<Mutex<mpsc::Receiver<Vec<u8>>>>,
    stream: &mut TcpStream,
    ws_tx: broadcast::Sender<String>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let (mut reader, mut writer) = stream.split();
    let mut buf = vec![0u8; 1024];

    loop {
        let mut rx_guard = rx.lock().await;
        tokio::select! {
            result = reader.read(&mut buf) => match result {
                Ok(0) => return Ok(()),
                Ok(n) => handle_server_data(&buf[..n], &ws_tx).await?,
                Err(e) => return Err(e.into()),
            },
            result = rx_guard.recv() => if let Some(data) = result {
                writer.write_all(&data).await?;
                writer.write_all(b"\n").await?;
                writer.flush().await?;
            } else {
                return Ok(());
            }
        }
    }
}

async fn connect_to_server(
    rx: Arc<Mutex<mpsc::Receiver<Vec<u8>>>>,
    ws_tx: broadcast::Sender<String>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    loop {
        println!("→ trying to connect to 127.0.0.1:8082…");
        match timeout(CONNECTION_TIMEOUT, TcpStream::connect("127.0.0.1:8082")).await {
            Ok(Ok(mut stream)) => {
                println!("✓ connected!");
                handle_stream(rx.clone(), &mut stream, ws_tx.clone()).await?
            }
            Ok(Err(e)) => {
                eprintln!("✗ connect error: {}", e);
                tokio::time::sleep(CONNECTION_RETRY_DELAY).await;
            }
            Err(_) => {
                eprintln!("✗ connect timed out after {:?}", CONNECTION_TIMEOUT);
                tokio::time::sleep(CONNECTION_RETRY_DELAY).await;
            }
        }
    }
}

async fn try_initial_connection(
    ws_tx: broadcast::Sender<String>,
    tcp_tx: Arc<Mutex<mpsc::Sender<Vec<u8>>>>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    match attempt_connection().await {
        Ok(mut stream) => {
            println!("Initial connection succeeded!");

            let (temp_tx, temp_rx) = mpsc::channel::<Vec<u8>>(CHANNEL_BUFFER_SIZE);

            {
                let mut guard = tcp_tx.lock().await;
                *guard = temp_tx;
            }
            handle_stream(Arc::new(Mutex::new(temp_rx)), &mut stream, ws_tx).await
        }
        Err(e) => {
            eprintln!("Initial connection failed: {}", e);
            Err(e)
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    println!("Starting server...");

    let base_path = std::env::var("SITE_URL")
        .map(|s| {
            let mut s = s.trim().to_string();
            if !s.is_empty() {
                if !s.starts_with('/') { s.insert(0, '/'); }
                if s.ends_with('/') && s != "/" { s.pop(); }
            }
            s
        })
        .unwrap_or_default();

    const ENABLE_K8S_CLIENT: bool = false;
    const ENABLE_INITIAL_CONNECTION: bool = false;
    const FORCE_REBUILD: bool = false;
    const BUILD_DOCKER_IMAGE: bool = true;
    const BUILD_DEPLOYMENT: bool = true;

    let (ws_tx, _) = broadcast::channel::<String>(CHANNEL_BUFFER_SIZE);
    let (tcp_tx, tcp_rx) = mpsc::channel::<Vec<u8>>(CHANNEL_BUFFER_SIZE);

    let mut client: Option<Client> = None;
    if ENABLE_K8S_CLIENT {
        client = Some(Client::try_default().await?);
    }

    let state = AppState {
        tcp_tx: Arc::new(Mutex::new(tcp_tx)),
        tcp_rx: Arc::new(Mutex::new(tcp_rx)),
        ws_tx: ws_tx.clone(),
        base_path: base_path.clone(),
        client,
    };

    if ENABLE_INITIAL_CONNECTION && state.client.is_some() {
        println!("Trying initial connection...");
        if try_initial_connection(ws_tx.clone(), state.tcp_tx.clone()).await.is_err() || FORCE_REBUILD {
            eprintln!("Initial connection failed or force rebuild enabled");
            if BUILD_DOCKER_IMAGE {
                docker::build_docker_image().await?;
            }
            if BUILD_DEPLOYMENT {
                kubernetes::create_k8s_deployment(state.client.as_ref().unwrap()).await?;
            }
        }
    }

    let bridge_rx = state.tcp_rx.clone();
    let bridge_tx = state.ws_tx.clone();
    tokio::spawn(async move {
        if let Err(e) = connect_to_server(bridge_rx, bridge_tx).await {
            eprintln!("Connection task failed: {}", e);
        }
    });

    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods([Method::GET, Method::POST])
        .allow_headers(Any);

    //let route1 = Router::new().route("/api/signin", post(sign_in));
    let inner = Router::new()
        .route("/api/message", get(get_message))
        .route("/api/send", post(receive_message))
        .route("/api/general", post(process_general))
        .route("/api/nodes", get(get_nodes))
        .route("/api/ws", get(ws_handler))
        .route("/api/signin", post(sign_in))
        //.merge(route1)
        .fallback_service(routes_static(Arc::new(state.clone())))
        .with_state(state.clone());

    let app = if base_path.is_empty() || base_path == "/" {
        inner.layer(cors)
    } else {
        Router::new().nest(&base_path, inner).layer(cors)
    };

    let addr: SocketAddr = "127.0.0.1:8081".parse().unwrap();
    println!("Listening on http://{}{}", addr, base_path);
    axum::serve(TcpListener::bind(addr).await?, app).await?;

    Ok(())
}

async fn ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_socket(socket, state))
}

async fn handle_socket(socket: WebSocket, state: AppState) {
    println!("WebSocket connected");
    let (mut sender, mut receiver) = socket.split();

    let mut broadcast_rx = state.ws_tx.subscribe();
    tokio::spawn(async move {
        while let Ok(msg) = broadcast_rx.recv().await {
            println!("Forwarding: {}", msg);
            if sender.send(Message::Text(msg.into())).await.is_err() {
                break;
            }
        }
    });

    while let Some(Ok(message)) = receiver.next().await {
        if let Message::Text(text) = message {
            println!("Got from client: {}", text);
            let payload = serde_json::from_str::<MessagePayload>(&text).unwrap_or(MessagePayload {
                r#type: "console".into(),
                message: text.to_string(),
                authcode: "0".into(),
            });

            if let Ok(mut bytes) = serde_json::to_vec(&payload) {
                bytes.push(b'\n');
                let _ = state.tcp_tx.lock().await.send(bytes).await;
            }
        }
    }

    println!("WebSocket disconnected");
}

async fn process_general(
    State(state): State<AppState>,
    Json(res): Json<ApiCalls>,
) -> Result<Json<ResponseMessage>, (StatusCode, String)> {
    let payload = res.standard_response().unwrap();
    println!("Processing general message: {:?}", payload);
    
    let json_payload = MessagePayload {
        r#type: payload.message_type.clone(),
        message: payload.message.clone(),
        authcode: payload.authcode.clone(),
    };

    match serde_json::to_vec(&json_payload) {
        Ok(mut json_bytes) => {
            json_bytes.push(b'\n');
 
            let tx = state.tcp_tx.clone();
            let tx_guard = tx.lock().await;
            
            match tx_guard.send(json_bytes).await {
                Ok(_) => {
                    println!("Successfully forwarded message to TCP server");
                    Ok(Json(ResponseMessage {
                        response: format!("Processed: {}", payload.message),
                    }))
                },
                Err(e) => {
                    eprintln!("Failed to send message to TCP channel: {}", e);
                    Err((StatusCode::INTERNAL_SERVER_ERROR,
                        "Failed to forward message to server".to_string()))
                }
            }
        }
        Err(e) => {
            eprintln!("Serialization error: {}", e);
            Err((StatusCode::BAD_REQUEST,
                "Invalid message format".to_string()))
        }
    }
}

async fn get_nodes(State(state): State<AppState>) -> impl IntoResponse {
    if state.client.is_some() {
        match kubernetes::list_node_names(state.client.unwrap()).await {
            Ok(nodes) => Json(List { list: nodes }),
            Err(err) => {
                eprintln!("Error listing nodes: {}", err);
                Json(List { list: vec![] })
            },
        }
    } else {
        Json(List { list: vec![] })
    }
}


async fn receive_message(
    State(state): State<AppState>,
    Json(res): Json<ApiCalls>,
) -> Result<Json<ResponseMessage>, (StatusCode, String)> {
    let payload = res.standard_response().unwrap();
    let json_payload = MessagePayload {
        r#type: payload.message_type.clone(),
        message: payload.message.clone(),
        authcode: payload.authcode.clone(),
    };

    match serde_json::to_vec(&json_payload) {
        Ok(mut json_bytes) => {
            json_bytes.push(b'\n'); 
            
            let tx_guard = state.tcp_tx.lock().await;
            match tx_guard.send(json_bytes).await {
                Ok(_) => Ok(Json(ResponseMessage {
                    response: format!("Successfully sent message: {}", payload.message),
                })),
                Err(e) => {
                    eprintln!("Failed to send message to TCP channel: {}", e);
                    Err((StatusCode::INTERNAL_SERVER_ERROR, 
                        "Failed to forward message to server".to_string()))
                }
            }
        }
        Err(e) => {
            eprintln!("Serialization error: {}", e);
            Err((StatusCode::BAD_REQUEST, 
                "Invalid message format".to_string()))
        }
    }
}

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

async fn handle_static_request(req: Request<Body>, state: Arc<AppState>) -> Response<Body> {
    let path = req.uri().path();
    let file = if path == "/" || path.is_empty() {
        "index.html"
    } else {
        &path[1..]
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


fn routes_static(state: Arc<AppState>) -> Router {
    let state_clone = state.clone();

    let public_api = Router::new()
        .route(
            "/api/",
            get(|| async { Json("This is a public API endpoint.") }),
        )
        .route("/", get({
            let state = state.clone();
            move |req| {
                let state = state.clone();
                async move { handle_static_request(req, state).await }
            }
        }));

    let fallback_router = Router::new()
        .fallback({
            let state = state_clone.clone();
            move |req| {
                let state = state.clone();
                async move { handle_static_request(req, state).await }
            }
        })
        .layer(middleware::from_fn(authorization_middleware));

    Router::new()
        .merge(public_api)     
        .merge(fallback_router) 
}




pub async fn authorization_middleware(mut req: Request, next: Next) -> Result<Response<Body>, WebErrors> {
    let auth_header = req.headers_mut().get(http::header::AUTHORIZATION);
    let auth_header = match auth_header {
        Some(header) => header.to_str().map_err(|_| WebErrors::AuthError {
            message: "Please add the JWT token to the header".to_string(),
            status_code: StatusCode::FORBIDDEN,
        })?,
        None => return Err(WebErrors::AuthError {
            message: "Please add a JWK token".to_string(),
            status_code: StatusCode::FORBIDDEN,
        })
    };
    let mut header = auth_header.split_whitespace();
    let (bearer, token) = (header.next(), header.next());
    let token = match resolve_jwt(token.unwrap().to_string()) {
        Ok(data) => data,
        Err(_) => return Err(WebErrors::AuthError { 
            message: "Failed to parse JWT token".to_string(), 
            status_code: StatusCode::UNAUTHORIZED, 
        })
    };

    let current_user = match retrive_user(token.claims.user){
        Some(user) => user,
        None => return Err(WebErrors::AuthError { 
            message: "You are not authorized".to_string(), 
            status_code: StatusCode::UNAUTHORIZED, 
        })
    };
    req.extensions_mut().insert(current_user);
    Ok(next.run(req).await)
}


pub fn verify_password(password: String, hash: String) -> Result<bool, bcrypt::BcryptError> {
    // Ok(true)
    verify(password, &hash)
}

pub async fn sign_in(
    State(state): State<AppState>,
    Json(res): Json<ApiCalls>,
) -> Result<Json<ResponseMessage>, StatusCode> {
    let user_data = res.login_data().unwrap();
    println!("{:#?}", user_data);
    let user = match retrive_user(user_data.user) {
        Some(user) => user,
        None => return Err(StatusCode::UNAUTHORIZED),
    };

   if !verify_password(user_data.password, user.password_hash)
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
   {
    return Err(StatusCode::UNAUTHORIZED)
   }

   let token = encode_token(user.username)
       .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(ResponseMessage { response: token }))
}

fn encode_token(user: String) -> Result<String, StatusCode>{
    let secret: String = "randomString".to_string();
    let now = chrono::Utc::now();
    let expire = OtherDuration::hours(24);
    let exp: usize = (now + expire).timestamp() as usize;
    let iat: usize = now.timestamp() as usize;
    let claim = Claims { 
        expirery: expire.num_seconds() as usize,
        iat, 
        user 
    };
    encode(&Header::default(), &claim, &EncodingKey::from_secret(secret.as_ref()))
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

fn retrive_user(user: String) -> Option<User>{
    let current_user: User = User {
        username: "e".to_string(),
        password_hash: "$2a$12$6fsBTxFz7gpHT60ZbwFU9OVywslC9zD0GbqznzYRUP4Oq6o5vI.1y".to_string(),
    };
    Some(current_user)
}
fn resolve_jwt(jwt: String) -> Result<TokenData<Claims>, StatusCode>{
    let secret = "randomString".to_string();
    let result: Result<TokenData<Claims>, StatusCode> = decode(
        &jwt, 
          &DecodingKey::from_secret(secret.as_ref()), 
   &Validation::default()
    )
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR);
    result

}

async fn get_message(
    State(state): State<AppState>,
) -> Result<Json<MessagePayload>, (StatusCode, String)> {
    let request = MessagePayload {
        r#type: "request".to_string(),
        message: "get_message".to_string(),
        authcode: "0".to_owned(),
    };

    let mut json_bytes = match serde_json::to_vec(&request) {
        Ok(mut v) => { v.push(b'\n'); v }
        Err(e) => {
            eprintln!("Serialization error: {}", e);
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to serialize request".into(),
            ));
        }
    };

    let tx_guard = state.tcp_tx.lock().await;
    if let Err(e) = tx_guard.send(json_bytes).await {
        eprintln!("Failed to send request: {}", e);
        return Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            "Failed to send request to server".into(),
        ));
    }
    drop(tx_guard);

    let mut rx_guard = state.tcp_rx.lock().await;
    match rx_guard.recv().await {
        Some(response_bytes) => {
            match serde_json::from_slice::<MessagePayload>(&response_bytes) {
                Ok(msg) => Ok(Json(msg)),
                Err(e) => {
                    eprintln!("Deserialization error: {}", e);
                    Err((
                        StatusCode::INTERNAL_SERVER_ERROR,
                        "Failed to parse server response".into(),
                    ))
                }
            }
        }
        None => {
            eprintln!("No response received");
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                "No response from server".into(),
            ))
        }
    }
}