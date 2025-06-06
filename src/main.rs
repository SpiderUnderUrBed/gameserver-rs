use std::collections::HashMap;
use std::convert::Infallible;
use std::fs;
use std::{net::SocketAddr, path::Path, sync::Arc};
//
use axum::extract::Query;
use axum::http::header::AUTHORIZATION;
use axum::http::Uri;
use axum::response::Redirect;
use axum::{Extension, Form};
use axum_login::tower_sessions::{MemoryStore, SessionManagerLayer};
use axum_login::{login_required, AuthManagerLayerBuilder, AuthUser, AuthnBackend, UserId};
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

use mime_guess::from_path;
use serde::{Deserialize, Serialize};
use serde_json::json;
use tokio::io::{AsyncBufReadExt, BufReader};
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
use bcrypt::BcryptError;
use async_trait::async_trait;
use tower_http::add_extension::AddExtensionLayer;
use axum::extract::FromRequest;


#[cfg(feature = "full-stack")]
mod docker;

#[cfg(feature = "full-stack")]
use kube::Client;

#[cfg(feature = "full-stack")]
mod kubernetes;

#[cfg(not(feature = "full-stack"))]
mod docker {
    pub async fn build_docker_image() -> Result<(), Box<dyn std::error::Error + Send + Sync>>{
        Err("This should not be running".into())
    }
}
#[cfg(not(feature = "full-stack"))]
mod kubernetes {
    pub async fn create_k8s_deployment(_: &crate::Client) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        Err("This should not be running".into())
    }
    pub async fn list_node_names(_: crate::Client) -> Result<Vec<String>, Box<dyn std::error::Error>> {
        Err("This should not be running".into())
    }
}
#[cfg(not(feature = "full-stack"))]
static TcpUrl: &str = "127.0.0.1:8082";

#[cfg(not(feature = "full-stack"))]
static LocalUrl: &str = "127.0.0.1:8081";

#[cfg(not(feature = "full-stack"))]
static K8S_WORKS: bool = false;

#[cfg(feature = "full-stack")]
static TcpUrl: &str = "gameserver-service:8080";

#[cfg(feature = "full-stack")]
static LocalUrl: &str = "127.0.0.1:8080";

#[cfg(feature = "full-stack")]
static K8S_WORKS: bool = true;

#[cfg(not(feature = "full-stack"))]
#[derive(Clone)]
struct Client;

#[cfg(not(feature = "full-stack"))]
impl Client {
    async fn try_default() -> Result<Self, Box<dyn std::error::Error + Send + Sync>>{
        Err("This should not be running".into())
    }
}

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

#[derive(Debug, Serialize, Deserialize)]
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


#[derive(Debug, Deserialize, Clone)]
struct IncomingMessage {
    message: String,
    #[serde(rename = "type")]
    message_type: String,
    authcode: String,
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
    timeout(CONNECTION_TIMEOUT, TcpStream::connect(TcpUrl)).await?.map_err(Into::into)
}

async fn handle_server_data(
    data: &[u8],
    ws_tx: &broadcast::Sender<String>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    if let Ok(text) = String::from_utf8(data.to_vec()) {
        println!("Raw message from server: {}", text);
        
        if let Ok(outer_msg) = serde_json::from_str::<InnerData>(&text) {
            let inner_data_str = outer_msg.data.as_str();
                if let Ok(inner_data) = serde_json::from_str::<serde_json::Value>(inner_data_str) {
                    if let Some(message_content) = inner_data["data"].as_str() {
                        println!("Extracted message: {}", message_content);
                        let _ = ws_tx.send(message_content.to_string());
                    }
                } else {
                    println!("Sending raw inner data: {}", inner_data_str);
                    let _ = ws_tx.send(inner_data_str.to_string());
                }
        } else if let Ok(_) = serde_json::from_str::<MessagePayload>(&text) {
            todo!()
        } else if let Ok(_) = serde_json::from_str::<List>(&text) {
            todo!()
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
    let mut buf_reader = BufReader::new(reader);
    let mut line = String::new();
    
    let msg = serde_json::to_string(
        &List {
            list: vec!["all".to_string()],
        }
    )? + "\n";
    
    writer.write_all(msg.as_bytes()).await?;
    match buf_reader.read_line(&mut line).await {
        Ok(0) => {
            println!("Error, possibly connection closed");
        }
        Ok(_) => {
            println!("Received line: {}", line.trim_end());
        }
        Err(e) => {
            println!("Error reading line: {:?}", e);
            return Err(e.into());
        }
    }
    reader = buf_reader.into_inner();

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
        println!("→ trying to connect to {}…", TcpUrl);
        match timeout(CONNECTION_TIMEOUT, TcpStream::connect(TcpUrl)).await {
            Ok(Ok(mut stream)) => {
                println!("connected!");
                handle_stream(rx.clone(), &mut stream, ws_tx.clone()).await?
            }
            Ok(Err(e)) => {
                eprintln!("connect error: {}", e);
                tokio::time::sleep(CONNECTION_RETRY_DELAY).await;
            }
            Err(_) => {
                eprintln!("connect timed out after {:?}", CONNECTION_TIMEOUT);
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

#[derive(Clone)]
struct AppState {
    tcp_tx: Arc<Mutex<mpsc::Sender<Vec<u8>>>>,
    tcp_rx: Arc<Mutex<mpsc::Receiver<Vec<u8>>>>,
    
    ws_tx: broadcast::Sender<String>,
    
    base_path: String,
    client: Option<Client>,
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

    const ENABLE_K8S_CLIENT: bool = true;
    const ENABLE_INITIAL_CONNECTION: bool = false;
    const FORCE_REBUILD: bool = false;
    const BUILD_DOCKER_IMAGE: bool = true;
    const BUILD_DEPLOYMENT: bool = true;

    let (ws_tx, _) = broadcast::channel::<String>(CHANNEL_BUFFER_SIZE);
    let (tcp_tx, tcp_rx) = mpsc::channel::<Vec<u8>>(CHANNEL_BUFFER_SIZE);

    let mut client: Option<Client> = None;
    if ENABLE_K8S_CLIENT && K8S_WORKS {
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

        let fallback_router = routes_static(state.clone().into());

        let inner = Router::new()
            .route("/api/message", get(get_message))
            .route("/api/send", post(receive_message))
            .route("/api/general", post(process_general))
            .route("/api/nodes", get(get_nodes))
            .route("/api/ws", get(ws_handler))
            .route("/api/signin", post(sign_in))
            .merge(fallback_router)
            .with_state(state.clone());
        

    let app = if base_path.is_empty() || base_path == "/" {
        inner.layer(cors)
    } else {
        Router::new().nest(&base_path, inner).layer(cors)
    };

    let addr: SocketAddr = LocalUrl.parse().unwrap();
    println!("Listening on http://{}{}", addr, base_path);

    // let addr: SocketAddr = "127.0.0.1:8081".parse().unwrap();
    // println!("Listening on http://{}{}", addr, base_path);
    // axum::serve(TcpListener::bind(addr).await?, app).await?;

    // Updated server start:
    let listener = TcpListener::bind(addr).await?;
    axum::serve(listener, app.into_make_service())
        .await?;

    Ok(())
}

fn routes_static(state: Arc<AppState>) -> Router<AppState> {
    let session_store = MemoryStore::default();
    let session_layer = SessionManagerLayer::new(session_store);

    let backend = Backend::default();
    let auth_layer = AuthManagerLayerBuilder::new(backend, session_layer).build();

    let public = Router::new()
        // .route("/login", get(login_page).post(sign_in))
        .route("/", get(handle_static_request))
        .route("/authenticate", get(authenticate_route))
        .route("/index.html", get(handle_static_request))
        .layer(AddExtensionLayer::new(state.clone()));

        let protected = Router::new()
        .route("/{*wildcard}", get(handle_static_request))
        .layer(AddExtensionLayer::new(state.clone()))
        .route_layer(login_required!(Backend, login_url = "/index.html"));

    public.merge(protected).route_layer(auth_layer)
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

pub type AuthSession = axum_login::AuthSession<Backend>;

#[derive(Deserialize, Serialize, Clone)]
pub struct Claims {
    pub exp: usize,
    pub iat: usize,
    pub user: String,
}

#[derive(Clone, Debug)]
pub struct User {
    pub username: String,
    pub password_hash: Option<String>,
}

impl AuthUser for User {
    type Id = String;

    fn id(&self) -> Self::Id {
        self.username.clone()
    }

    fn session_auth_hash(&self) -> &[u8] {
        self.username.as_bytes()
    }
}

#[derive(Clone, Default)]
pub struct Backend {
    pub users: HashMap<String, User>,
}

#[async_trait]
impl AuthnBackend for Backend {
    type User = User;
    type Credentials = String;
    type Error = Infallible;

    async fn authenticate(&self, token: Self::Credentials) -> Result<Option<Self::User>, Self::Error> {
        let user = resolve_jwt(&token).ok().map(|data| User {
            username: data.claims.user,
            password_hash: None,
        });
        Ok(user)
    }

    async fn get_user(&self, user_id: &String) -> Result<Option<Self::User>, Self::Error> {
        Ok(Some(User {
            username: user_id.clone(),
            password_hash: None,
        }))
    }
}

const SECRET: &str = "randomString";

fn resolve_jwt(token: &str) -> Result<TokenData<Claims>, StatusCode> {
    decode::<Claims>(
        token,
        &DecodingKey::from_secret(SECRET.as_bytes()),
        &Validation::default(),
    ).map_err(|_| StatusCode::UNAUTHORIZED)
}

fn encode_token(user: String) -> Result<String, StatusCode> {
    let now = Utc::now();
    let exp = (now + chrono::Duration::hours(24)).timestamp() as usize;
    let iat = now.timestamp() as usize;
    let claims = Claims { exp, iat, user };

    encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(SECRET.as_bytes()),
    ).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

#[derive(Debug, Deserialize, Clone)]
pub struct LoginData {
    pub user: String,
    pub password: String,
}

#[derive(Deserialize)]
pub struct JwtTokenForm {
    pub token: String,
}

#[axum::debug_handler]
pub async fn sign_in(
    State(_): State<AppState>,
    Form(request): Form<LoginData>
) -> Result<Json<ResponseMessage>, StatusCode> {
    let user = retrive_user(request.user.clone()).ok_or(StatusCode::UNAUTHORIZED)?;

    let password_valid = verify_password(request.password, user.password_hash.unwrap())
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    if !password_valid {
        return Err(StatusCode::UNAUTHORIZED);
    }

    let token = encode_token(user.username)?;
    Ok(Json(ResponseMessage  { response: token }))
}

pub fn retrive_user(username: String) -> Option<User> {
    if username == "testuser" {
        let password_hash = bcrypt::hash("password123", bcrypt::DEFAULT_COST).ok();
        Some(User {
            username,
            password_hash,
        })
    } else {
        None
    }
}

pub fn verify_password(password: String, hash: String) -> Result<bool, bcrypt::BcryptError> {
    bcrypt::verify(password, &hash)
}

async fn serve_html_with_replacement(
    file: &str,
    state: &AppState,
) -> Result<Response<Body>, StatusCode> {
    let path = Path::new("src/vanilla/html").join(file);

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

async fn handle_static_request(
    Extension(state): Extension<Arc<AppState>>,
    req: Request<Body>,
) -> Result<Response<Body>, StatusCode> {
 
    let path = req.uri().path();
    let file = if path == "/" || path.is_empty() {
        "index.html"
    } else {
        &path[1..]
    };

    match serve_html_with_replacement(file, &state).await {
        Ok(res) => Ok(res),
        Err(status) => Ok(Response::builder()
            .status(status)
            .header("content-type", "text/plain")
            .body(format!("Error serving `{}`", file).into())
            .unwrap()),
    }
}

#[derive(Deserialize)]
pub struct AuthenticateParams {
    next: String,
    jwk: String,
}

async fn authenticate_route(
    State(_state): State<AppState>,
    Query(params): Query<AuthenticateParams>,
    mut auth_session: AuthSession,
) -> impl IntoResponse {
    match resolve_jwt(&params.jwk) {
        Ok(token_data) => {
            let user = User {
                username: token_data.claims.user,
                password_hash: None,
            };

            if let Err(e) = auth_session.login(&user).await {
                eprintln!("Failed to log in user: {:?}", e);
                return (StatusCode::INTERNAL_SERVER_ERROR, "Failed to log in").into_response();
            }

            if params.next.starts_with('/') {
                if let Ok(uri) = params.next.parse::<Uri>() {
                    return Redirect::to(&uri.to_string()).into_response();
                } else {
                    return (StatusCode::BAD_REQUEST, "Invalid next parameter: unable to parse URI").into_response();
                }
            } else {
                return (StatusCode::BAD_REQUEST, "Invalid next parameter: must start with '/'").into_response();
            }
        }
        Err(_) => {
            (StatusCode::UNAUTHORIZED, "Invalid token").into_response()
        }
    }
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
