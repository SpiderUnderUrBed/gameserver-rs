// ALOT of imports, needed given the size of this project in what it covers
// first imports are std ones
use std::collections::HashMap;
use std::convert::Infallible;
use std::fmt::Debug;
use std::{net::SocketAddr, path::Path, sync::Arc};

// Axum is the routing framework, and the backbone to this project helping intergrate the backend with the frontend
// and the general api, redirections, it will take form data and queries and make it easily accessible
// I also use axum_login to take off alot of effort that would be required for authentication
use crate::database::Element;
use crate::filesystem::{send_multipart_over_broadcast, FsType};
use crate::http::HeaderMap;
use crate::kubernetes::verify_is_k8s_gameserver;
use crate::middleware::from_fn;
use axum::extract::ws::Message as WsMessage;
use axum::extract::Multipart;
use axum::extract::Query;
use axum::http::Uri;
use axum::middleware::{self, Next};
use axum::response::Redirect;
use axum::Form;
use axum::{
    body::Body,
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        Request, State,
    },
    http::{self, Method, Response, StatusCode},
    response::{
        sse::{Event, Sse},
        Html, IntoResponse, Json,
    },
    routing::{get, post},
    Router,
};
use axum_login::tower_sessions::{MemoryStore, SessionManagerLayer};
use axum_login::AuthUser;
use axum_login::{AuthManagerLayerBuilder, AuthnBackend};
use serde::de::DeserializeOwned;
use tokio::fs::File;
use tokio::sync::RwLock;

// mod databasespec;
// use databasespec::UserDatabase;
use crate::database::databasespec::{Button, NodeStatus};
use crate::database::databasespec::ButtonsDatabase;
use crate::database::databasespec::NodesDatabase;
use crate::database::databasespec::UserDatabase;
use crate::database::Node;
use crate::database::databasespec::NodeType;
// miscellancious imports, future traits are used because alot of the code is asyncronus and cant fully be contained in tokio
// mime_guess as when I am serving the files, I need to serve it with the correct mime type
// serde_json because I exchange alot of json data between the backend and frontend and to the gameserver
// tokio because when working with alot of networking stuff and things that will take a indeterminent amount of time, async/await is the way to go (for better efficency too)
// chrono for time, tower for cors (TODO:: use less permissive CORS due to potential security risks)
// jsonwebtokens is standard when working with authentication, and bcrypt so I can use password hashs, I explain the authentication methods later
use async_trait::async_trait;
use chrono::{Duration as OtherDuration, Utc};
use futures_util::{sink::SinkExt, stream::StreamExt};
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, TokenData, Validation};
use mime_guess::from_path;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use serial_test::serial;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::{
    fs as tokio_fs,
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    sync::{broadcast, Mutex},
    time::{timeout, Duration},
};
use tower_http::cors::{Any as CorsAny, CorsLayer};

use futures_util::{stream, Stream, TryFutureExt};

use std::sync::atomic::{AtomicUsize, Ordering};
use tokio_util::bytes::Bytes;
static CONNECTION_COUNTER: AtomicUsize = AtomicUsize::new(0);
use tokio::time::interval;

// For now I only restrict the json backend for running this without kubernetes
// the json backend is only for testing in most cases, simple deployments would use full-stack feature flag
// and you can use postgres manually with the database feature flag
#[cfg(any(feature = "full-stack", feature = "docker", feature = "database"))]
mod database {
    include!("pgdatabase.rs");
}

#[cfg(all(not(feature = "full-stack"), not(feature = "docker"), not(feature = "database")))]
mod database {
    include!("jsondatabase.rs");
}

// JsonDatabase is only something that would be unique to Json and not any other database managed by sqlx
#[cfg(all(not(feature = "full-stack"), not(feature = "docker"), not(feature = "database")))]
use database::JsonBackend;

// Both database files and any more should have these structs
use database::ModifyElementData;
use database::RetrieveUser;
use database::User;

mod extra;
use extra::{value_from_line};

mod filesystem;
use filesystem::{
    FileChunk, FileResponseMessage, FsEntry, FsItem, FsMetadata, RemoteFileSystem, TcpFs,
};

// Docker AND kubernetes would be enabled with a standard deployment
// as you wouldnt need the docker module (or the k8s module) for barebones testing
#[cfg(feature = "full-stack")]
mod docker;

#[cfg(feature = "full-stack")]
mod kubernetes;

// Main has to store the client, so I would remove the client here if this is not in a standard deployment in favor
// of a dummy one
#[cfg(feature = "full-stack")]
use kube::Client;

// build_docker_image and the functions from the kubernetes modules needs to be faked to make the compiler happy if this is not a standard deployment
#[cfg(not(feature = "full-stack"))]
mod docker {
    pub async fn build_docker_image() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        Err("This should not be running".into())
    }
}
#[cfg(not(feature = "full-stack"))]
mod kubernetes {
    use crate::NodeAndTCP;

    pub async fn create_k8s_deployment(
        _: &crate::Client,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        Err("This should not be running".into())
    }
    pub async fn list_node_names(
        _: crate::Client,
    ) -> Result<Vec<String>, Box<dyn std::error::Error>> {
        Err("This should not be running".into())
    }
    pub async fn list_node_info(
    _: crate::Client,
    ) -> Result<Vec<NodeAndTCP>, Box<dyn std::error::Error + Send + Sync>> {
        Err("This should not be running".into())
    }
    pub async fn verify_is_k8s_gameserver(
    _: crate::Client,
    _: String
    ) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
        Ok(false)
    }
    pub async fn get_avalible_gameserver(
    _: &crate::Client,
    ) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        Err("This should not be running in a non-k8s environemnt".into())
    }
}

// This part would potentially be removed later
// I like these defaults for testing, and for the moment I doubt anyone would object
// but at some point this will be removed in favor of testing with ENV varibles
#[cfg(not(feature = "full-stack"))]
static TcpUrl: &str = "127.0.0.1:8082";

#[cfg(not(feature = "full-stack"))]
static LocalUrl: &str = "127.0.0.1:8081";

#[cfg(not(feature = "full-stack"))]
static K8S_WORKS: bool = false;

#[cfg(not(feature = "docker"))]
static DOCKER_WORKS: bool = false;

#[cfg(feature = "full-stack")]
static TcpUrl: &str = "gameserver-service:8080";

#[cfg(feature = "full-stack")]
static LocalUrl: &str = "127.0.0.1:8080";

static WEBSOCKET_DEBUGGING: bool = false;
// K8S_WORKS needs to be true in the case where the full stack is running and not if that is not the case
// to avoid calling the dummy functions
#[cfg(feature = "full-stack")]
static K8S_WORKS: bool = true;

#[cfg(feature = "docker")]
static DOCKER_WORKS: bool = true;

// dummy client and function
#[cfg(not(feature = "full-stack"))]
#[derive(Clone)]
struct Client;

#[cfg(not(feature = "full-stack"))]
impl Client {
    async fn try_default() -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        Err("This should not be running".into())
    }
}

// The database connection would be avalible in the full-stack or explicit database testing
// which in this case means postgres
#[cfg(any(feature = "full-stack", feature = "docker", feature = "database"))]
async fn first_connection() -> Result<sqlx::Pool<sqlx::Postgres>, sqlx::Error> {
    // The user should be able to customize alot about where the database is, how to authenticate with it,
    // whether it is being ran with the full stack or not, hence the env varibles with sensible defaults
    let db_user = std::env::var("POSTGRES_USER").unwrap_or("gameserver".to_string());
    let db_password = std::env::var("POSTGRES_PASSWORD").unwrap_or("gameserverpass".to_string());
    let db = std::env::var("POSTGRES_DB").unwrap_or("gameserver_db".to_string());
    let db_port = std::env::var("POSTGRES_PORT").unwrap_or("5432".to_string());
    let db_host = std::env::var("POSTGRES_HOST").unwrap_or("gameserver-postgres".to_string());

    // initial connection which is returned
    sqlx::postgres::PgPool::connect(&format!(
        "postgres://{}:{}@{}:{}/{}",
        db_user, db_password, db_host, db_port, db
    ))
    .await
}

// for the default testing environment, it should be json
// due to reduced complexity, and currently at the time of writing this
// dependency issues, so unless you are testing the postgres db itself with this project
// the json backend MIGHT be sufficent, but at the time of writing this I have not made the json backend work
#[cfg(all(not(feature = "full-stack"), not(feature = "docker"), not(feature = "database")))]
async fn first_connection() -> Result<JsonBackend, String> {
    Ok(JsonBackend::new(None))
}

// varibles which determines stuff about the tcp connection to the gameserver for data exchange
const CONNECTION_RETRY_DELAY: Duration = Duration::from_secs(2);
const CONNECTION_TIMEOUT: Duration = Duration::from_secs(3);
const CHANNEL_BUFFER_SIZE: usize = 32;

const ALLOW_NONJSON_DATA: bool = false;

// MessagePayload is how most data are exchanged between the gameserver, and the frontend
#[derive(Debug, Serialize, Deserialize)]
struct MessagePayload {
    r#type: String,
    message: String,
    authcode: String,
}

// #[derive(PartialEq)]

struct NodeAndTCP {
    name: String, 
    ip: String, 
    nodetype: NodeType,
    tcp_tx: Option<tokio::sync::broadcast::Sender<Vec<u8>>>,
    tcp_rx: Option<tokio::sync::broadcast::Receiver<Vec<u8>>>
}
impl Clone for NodeAndTCP {
    fn clone(&self) -> NodeAndTCP {
        NodeAndTCP {
            name: self.name.clone(),
            ip: self.ip.clone(),
            nodetype: self.nodetype.clone(),
            tcp_tx: self.tcp_tx.clone(),
            tcp_rx: self.tcp_tx.as_ref().map(|tx| tx.subscribe()),
        }
    }
}

// console messages are for the console specifically, but this might be redundant
#[derive(Debug, Deserialize)]
struct ConsoleMessage {
    data: String,
    #[serde(rename = "type")]
    message_type: String,
    authcode: String,
}

// console output is sometimes contained within the data feild of json, but this also might be redundant
#[derive(Debug, Deserialize, Serialize)]
struct InnerData {
    data: String,
    #[serde(rename = "type")]
    message_type: String,
    authcode: String,
}

// A simple response message, no authorization needed, alhough the equivalent would be setting the authcode to 0 and making a custom type for this
#[derive(Debug, Serialize)]
struct ResponseMessage {
    response: String,
}

// a list for things like nodes, capabilities, etc
#[derive(Debug, Serialize, Deserialize)]
struct List {
    list: ApiCalls,
}

// some custom web error messages to be used in the frontend, graciously provided by axum
enum WebErrors {
    AuthError {
        message: String,
        status_code: StatusCode,
    },
}
impl IntoResponse for WebErrors {
    fn into_response(self) -> Response<Body> {
        let (status_code, message) = match self {
            WebErrors::AuthError {
                message,
                status_code,
            } => (status_code, message),
        };

        Response::builder()
            .status(status_code)
            .header("content-type", "application/json")
            .body(Body::from(
                serde_json::to_string(&json!({ "error": message })).unwrap(),
            ))
            .unwrap()
    }
}
#[derive(Debug, serde::Deserialize)]
struct KeywordPayload {
    start_keyword: Option<String>,
    stop_keyword: Option<String>,
}

// May be redundant, but this is a struct for incoming messages
#[derive(Debug, Deserialize, Serialize, Clone)]
struct IncomingMessage {
    message: String,
    #[serde(rename = "type")]
    message_type: String,
    authcode: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
struct SrcAndDest {
    src: ApiCalls,
    dest: ApiCalls,
    metadata: String
}
struct BrowserErrMessage {
    err: String
}

// Some common api calls which is just what might get exchanged between the frontend and backend via api
// this is needed rather than a bunch of structs or however else I might do it because in some cases I might not know what api call to expect
// as it would be determined by a 'kind' flag provided by serde, and the content, be it a array or struct, be nested in json (which provides new hurdles for how to process
// data as I cant EXPECT JSON in there)
#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(tag = "kind", content = "data")]
enum ApiCalls {
    None,
    Capabilities(Vec<String>),
    NodeList(Vec<String>),
    UserData(LoginData),
    UserList(Vec<User>),
    ButtonList(Vec<Button>),
    IncomingMessage(IncomingMessage),
    FileList(Vec<FsItem>),
    Node(Node),
}

#[derive(Clone)]
enum Status {
    Up,
    Healthy,
    Down,
    Unhealthy,
}

// AppState, this is a global struct which will be used to store data needed across the application like in routes and etc
// which includes the sender and reciver to the tcp connection for gameserver, the websocket sender (receiver only needs to be managed by its own handler)
// the base path like if all the routes are prefixed with something like /gameserver-rs which is the default for my testing deployment, and database as its needed frequently
// for user information and etc
// #[derive(Clone)]
struct AppState {
    tcp_tx: tokio::sync::broadcast::Sender<Vec<u8>>,
    tcp_rx: tokio::sync::broadcast::Receiver<Vec<u8>>,
    additonal_node_tcp: Vec<NodeAndTCP>,
    //additonal_node_tcp: Vec<(tokio::sync::broadcast::Sender<Vec<u8>>, tokio::sync::broadcast::Receiver<Vec<u8>>)>,
    gameserver: Value,
    status: Status,
    ws_tx: broadcast::Sender<String>,
    base_path: String,
    client: Option<Client>,
    database: database::Database,
}
impl Clone for AppState {
    fn clone(&self) -> Self {
        AppState {
            tcp_tx: self.tcp_tx.clone(),
            tcp_rx: self.tcp_rx.resubscribe(),
            gameserver: self.gameserver.clone(),
            status: self.status.clone(),
            ws_tx: self.ws_tx.clone(),
            base_path: self.base_path.clone(),
            client: self.client.clone(),
            database: self.database.clone(),
            additonal_node_tcp: self.additonal_node_tcp
                .iter()
                .map(|node| 
                    NodeAndTCP {
                        name: node.name.clone(),
                        ip: node.ip.clone(),
                        tcp_tx: node.tcp_tx.clone(),
                        tcp_rx: {
                            if let Some (tcp_rx) = &node.tcp_rx {
                                Some(tcp_rx.resubscribe())
                            } else {
                                None
                            }
                        },
                        nodetype: node.nodetype.clone(),
                    }
                )
                .collect(),
        }
    }
}


// for the initial connection attempt, which will determine if possibly I would need to create the container and deployment upon failure
// i will use rusts 'timeout' for x interval determined with CONNECTION_TIMEOUT
async fn attempt_connection(tcp_url: String) -> Result<TcpStream, Box<dyn std::error::Error + Send + Sync>> {
    timeout(CONNECTION_TIMEOUT, TcpStream::connect(tcp_url))
        .await?
        .map_err(Into::into)
}

// use tokio::sync::broadcast::error::RecvError::Lagged;
// use tokio::sync::broadcast::error::RecvError::Closed;
pub async fn handle_stream(
    arc_state: Arc<RwLock<AppState>>,
    rx: &mut tokio::sync::broadcast::Receiver<Vec<u8>>,
    stream: &mut TcpStream,
    ws_tx: broadcast::Sender<String>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let ip = stream.peer_addr()?.to_string();
    let (reader, mut writer) = stream.split();
    let mut buf_reader = BufReader::new(reader);
    let mut buf = vec![0u8; 4096];

    let capability_msg = serde_json::to_string(&List {
        list: ApiCalls::Capabilities(vec!["all".to_string()]),
    })? + "\n";
    writer.write_all(capability_msg.as_bytes()).await?;

    for command in &["server_data", "server_name"] {
        let cmd_msg = serde_json::to_string(&MessagePayload {
            r#type: "command".to_string(),
            message: command.to_string(),
            authcode: "0".to_string(),
        })? + "\n";
        writer.write_all(cmd_msg.as_bytes()).await?;
    }

    let mut server_start_keyword = String::new();
    let mut server_stop_keyword = String::new();

    loop {
        tokio::select! {
            read_result = buf_reader.read(&mut buf) => {
                match read_result {
                    Ok(0) => break,
                    Ok(n) => {
                        let raw_data = &buf[..n];
                        {
                            let state_guard = arc_state.read().await;
                            let _ = state_guard.tcp_tx.send(raw_data.to_vec());
                        }

                        if let Ok(text) = std::str::from_utf8(raw_data) {
                            let line_content = text.trim();
                            if line_content.is_empty() { continue; }

                            let mut final_data: Vec<ConsoleData> = vec![];

                            let list_parsed: Vec<Result<List, serde_json::Error>> =
                                value_from_line::<List, _>(line_content, |line| line.contains("\"list\"")).await;

                            let mut list_values: Vec<ConsoleData> = vec![];
                            for item in list_parsed {
                                if let Ok(list_item) = item {
                                    let serialized = serde_json::to_string(&list_item)?;
                                    list_values.push(ConsoleData {
                                        data: serialized,
                                        r#type: "list_item".to_string(),
                                        authcode: "0".to_string(),
                                    });
                                }
                            }
                            let mut list_lines: Vec<String> = list_values.iter().map(|v| v.data.clone()).collect();

                            let console_parsed: Vec<Result<ConsoleData, serde_json::Error>> =
                                value_from_line::<ConsoleData, _>(line_content, |line| !line.contains("\"list\"")).await;

                            let mut console_values: Vec<ConsoleData> = vec![];
                            for item in console_parsed {
                                if let Ok(data) = item {
                                    if !list_lines.contains(&data.data) {
                                        console_values.push(data);
                                    }
                                }
                            }

                            final_data.extend(list_values);
                            final_data.extend(console_values);

                            for data_clone in final_data.iter() {
                                if let Ok(inner) = serde_json::from_str::<serde_json::Value>(&data_clone.data) {
                                    if let (Some(start_kw), Some(stop_kw)) = (
                                        inner.get("start_keyword").and_then(|v| v.as_str()),
                                        inner.get("stop_keyword").and_then(|v| v.as_str()),
                                    ) {
                                        server_start_keyword = start_kw.to_string();
                                        server_stop_keyword = stop_kw.to_string();
                                    }
                                }

                                if data_clone.data.contains("\"type\":\"command\"") {
                                    if let Ok(inner_msg) = serde_json::from_str::<MessagePayload>(&data_clone.data) {
                                        if inner_msg.r#type == "command" {
                                            let (client_option, database) = {
                                                let state_guard = arc_state.read().await;
                                                (state_guard.client.clone(), state_guard.database.clone())
                                            };

                                            if let Ok(nodes) = database.fetch_all_nodes().await {
                                                let node_status = if let Some(client) = client_option {
                                                    let client_clone = client.clone();
                                                    let ip_clone = ip.clone();
                                                    
                                                    match tokio::time::timeout(
                                                        std::time::Duration::from_millis(100),
                                                        verify_is_k8s_gameserver(client_clone, ip_clone)
                                                    ).await {
                                                        Ok(Ok(true)) => NodeStatus::ImmutablyEnabled,
                                                        _ => NodeStatus::Enabled,
                                                    }
                                                } else {
                                                    NodeStatus::Enabled
                                                };

                                                let node = Node {
                                                    ip: ip.clone(),
                                                    nodename: inner_msg.message,
                                                    nodetype: NodeType::Custom,
                                                    nodestatus: node_status,
                                                };

                                                if !nodes.iter().any(|n| n.ip == node.ip && n.nodename == node.nodename) {
                                                    let _ = database.create_nodes_in_db(ModifyElementData {
                                                        element: Element::Node(node),
                                                        jwt: "".to_string(),
                                                        require_auth: false,
                                                    }).await;
                                                }
                                            }
                                        }
                                    }
                                }

                                if data_clone.data.contains("\"type\":\"stdout\"") {
                                    if let Ok(output_msg) = serde_json::from_str::<serde_json::Value>(&data_clone.data) {
                                        if let Some(server_output) = output_msg.get("data").and_then(|v| v.as_str()) {
                                            if !server_start_keyword.is_empty() && server_output.contains(&server_start_keyword) {
                                                let _ = ws_tx.send("Server is ready for connections!".to_string());
                                                {
                                                    let mut state_guard = arc_state.write().await;
                                                    state_guard.status = Status::Up;
                                                }
                                            } else if !server_stop_keyword.is_empty() && server_output.contains(&server_stop_keyword) {
                                                {
                                                    let mut state_guard = arc_state.write().await;
                                                    state_guard.status = Status::Down;
                                                }
                                            }
                                            let _ = ws_tx.send(server_output.to_string());
                                            continue;
                                        }
                                    }
                                }

                                if !data_clone.data.contains("\"type\":\"stdout\"") && !data_clone.data.contains("\"type\":\"command\"") {
                                    let _ = ws_tx.send(data_clone.data.clone());
                                }
                            }
                        }
                    }
                    Err(e) => return Err(e.into()),
                }
            }

            broadcast_result = rx.recv() => {
                match broadcast_result {
                    Ok(data) => {
                        writer.write_all(&data).await?;
                        writer.write_all(b"\n").await?;
                        writer.flush().await?;
                    }
                    Err(_) => {}
                }
            }
        }
    }

    Ok(())
}

// use tokio::sync::broadcast::error::RecvError::Lagged;
// use tokio::sync::broadcast::error::RecvError::Closed;


// does the connection to the tcp server, wether initial or not, on success it will pass it off to the dedicated handler for the stream
async fn connect_to_server(
    state: Arc<RwLock<AppState>>,
    tcp_url: String,
    mut rx: tokio::sync::broadcast::Receiver<Vec<u8>>,
    ws_tx: broadcast::Sender<String>,
) -> Result<SocketAddr, Box<dyn std::error::Error + Send + Sync>> {
    loop {
        match timeout(CONNECTION_TIMEOUT, TcpStream::connect(tcp_url.clone())).await {
            Ok(Ok(mut stream)) => {
                let stream_result =
                    handle_stream(state.clone(), &mut rx, &mut stream, ws_tx.clone()).await;
                if stream_result.is_ok() {
                    return Ok(stream.peer_addr()?);
                } else if let Err(error) = stream_result {
                    return Err(error);
                }
            }
            Ok(Err(e)) => {
                tokio::time::sleep(CONNECTION_RETRY_DELAY).await;
            }
            Err(_) => {
                tokio::time::sleep(CONNECTION_RETRY_DELAY).await;
            }
        }
    }
}
// This function is soley for the initial connection to the tcp server, then passes it off to the dedicated handler, for the initial conneciton
// this is where it determines wether or not to try and create the container and deployment, as attempt_connection itself is used in various diffrent contexts (like it will constantly
// try to connect upon failing but it should not try to create the container and deployment every time it fails)
async fn try_initial_connection(
    state: Arc<RwLock<AppState>>,
    tcp_url: String,
    ws_tx: broadcast::Sender<String>,
    tcp_tx: tokio::sync::broadcast::Sender<Vec<u8>>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    match attempt_connection(tcp_url).await {
        Ok(mut stream) => {
            println!("Initial connection succeeded!");

            let (temp_tx, temp_rx) =
                tokio::sync::broadcast::channel::<Vec<u8>>(CHANNEL_BUFFER_SIZE);

            let mut temp_rx = temp_rx; // make receiver mutable
            handle_stream(state, &mut temp_rx, &mut stream, ws_tx).await
        }
        Err(e) => {
            eprintln!("Initial connection failed: {}", e);
            Err(e)
        }
    }
}

// main function handles the initial connection
// initilizing the database struct, getting and setting the base path as well as alot of defaults in AppState
// trying the initial tcp connection to gameserver, and considering creating it if it doesnt exist, and will continually try to make a connection with it
// until successful, then it will serve the webserver (maybe the pinging for gameserver should not be a requirement for the webserver to run)
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    println!("Starting server...");

    let conn = first_connection().await?;
    let database = database::Database::new(Some(conn));

    let verbose = std::env::var("VERBOSE").is_ok();
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

    // Overrides for testing or specific cases where how it worksin a setup may be diffrent
    const ENABLE_K8S_CLIENT: bool = true;
    const ENABLE_INITIAL_CONNECTION: bool = false;
    const FORCE_REBUILD: bool = false;
    const BUILD_DOCKER_IMAGE: bool = true;
    const BUILD_DEPLOYMENT: bool = true;
    const DONT_OVERRIDE_CONN_WITH_K8S: bool = false;

    // creates a websocket broadcase and tcp channels
    let (ws_tx, _) = broadcast::channel::<String>(CHANNEL_BUFFER_SIZE);
    let (tcp_tx, tcp_rx) = broadcast::channel::<Vec<u8>>(CHANNEL_BUFFER_SIZE);

    // sets the client to be none by default unless this is ran the stanard way which will be ran with the appropriate feature-flag
    // which will set the k8s client
    let mut client: Option<Client> = None;
    if ENABLE_K8S_CLIENT && K8S_WORKS {
        client = Some(Client::try_default().await?);
    }

    let mut tcp_url: String = TcpUrl.to_string();
    if !DONT_OVERRIDE_CONN_WITH_K8S && client.is_some() {
        if let Ok(url_result) = &kubernetes::get_avalible_gameserver(client.as_ref().unwrap()).await {
            tcp_url = url_result.clone();  
        } else {
            println!("Could not get a successful url for a existing gameserver, will try the fallback url")
        }
    }


    let mut nodes: Vec<NodeAndTCP> = vec![];
    if let Ok(db_nodes) = database.fetch_all_nodes().await {
        nodes = db_nodes.into_iter().map(|node| NodeAndTCP { name: node.nodename, nodetype: node.nodetype, ip: node.ip, tcp_tx: None, tcp_rx: None }).collect()
    }

    // use everything so far to make the app state
    let state: AppState = AppState {
        gameserver: json!({}),
        status: Status::Down,
        tcp_tx: tcp_tx,
        // tcp_rx: Arc::new(Mutex::new(tcp_rx)),
        tcp_rx: tcp_rx,
        ws_tx: ws_tx.clone(),
        base_path: base_path.clone(),
        database,
        client,
        additonal_node_tcp: nodes
    };

    let multifaceted_state = Arc::new(RwLock::new(state));

    // if there is supposed to be a initial connection and if there is a client (as it wont be able to create the deployment without it, and it would be pointless to create a docker container
    // without the abbility to deploy it)
    if ENABLE_INITIAL_CONNECTION && multifaceted_state.write().await.client.is_some() {
        println!("Trying initial connection...");
        if try_initial_connection(
            multifaceted_state.clone(),
            tcp_url.to_string(),
            ws_tx.clone(),
            multifaceted_state.write().await.tcp_tx.clone(),
        )
        .await
        .is_err()
            || FORCE_REBUILD
        {
            eprintln!("Initial connection failed or force rebuild enabled");
            if BUILD_DOCKER_IMAGE {
                docker::build_docker_image().await?;
            }
            if BUILD_DEPLOYMENT {
                kubernetes::create_k8s_deployment(
                    multifaceted_state.write().await.client.as_ref().unwrap(),
                )
                .await?;
            }
        }
    }

    // takes the tcp connection out of the arc mutex and gets a connection to the
    // websocket to send to connect_to_server to establish the date pipeline
    let server_connection_state = multifaceted_state.clone();
    let bridge_rx = multifaceted_state.read().await.tcp_rx.resubscribe();
    let bridge_tx = multifaceted_state.read().await.ws_tx.clone();

    tokio::spawn(async move {
        if let Err(e) = connect_to_server(server_connection_state, tcp_url.to_string(), bridge_rx, bridge_tx).await {
            eprintln!("[DEBUG] Connection task failed: {}", e);
        }
    });

    // Currently very permissive CORS for permissions
    let cors = CorsLayer::new()
        .allow_origin(CorsAny)
        .allow_methods([Method::GET, Method::POST])
        .allow_headers(CorsAny);

    // fallback_router will serve all the basic files
    let fallback_router = routes_static(multifaceted_state.clone());

    // the main route, this serves all the api stuff that wont be behind a login, but I handle the main routes in routes_static for better control
    // over the authentication flow, if the api could be publically accessible in the future, you would need a diffrent way to authenticate with a api
    //let global_state = multifaceted_state.write().await.clone();
    let inner = Router::new()
        .route("/api/message", get(get_message))
        .route("/api/nodes", get(get_nodes))
        .route("/api/buttons", get(get_buttons))
        .route("/api/servers", get(get_servers))
        .route("/api/users", get(users))
        .route("/api/ws", get(ws_handler))
        // .route("getfiles", get(get_files))
        .route("/api/upload", post(upload))
        //.route("/api/uploadcontent", post(upload))
        .route("/api/awaitserverstatus", get(ongoing_server_status))
        .route("/api/fetchnode", post(fetch_node))
        .route("/api/migrate", post(migrate))
        .route("/api/getstatus", post(get_status))
        .route("/api/getfiles", post(get_files))
        .route("/api/getfilescontent", post(get_files_content))
        .route("/api/buttonreset", post(button_reset))
        .route("/api/editbuttons", post(edit_buttons))
        .route("/api/addnode", post(add_node))
        .route("/api/edituser", post(edit_user))
        .route("/api/getuser", post(get_user))
        .route("/api/getserver", post(get_server))
        .route("/api/send", post(receive_message))
        .route("/api/general", post(process_general))
        .route("/api/signin", post(sign_in))
        .route("/api/createuser", post(create_user))
        .route("/api/deleteuser", post(delete_user))
        .merge(fallback_router)
        .with_state(multifaceted_state.clone());

    let normal_routes = Router::new().merge(inner);

    // Does nesting of routes behind a base path if configured, otherwise use defaults
    let app = if base_path.is_empty() || base_path == "/" {
        normal_routes.layer(cors)
    } else {
        Router::new().nest(&base_path, normal_routes).layer(cors)
    };

    // serves the website
    let addr: SocketAddr = LocalUrl.parse().unwrap();
    println!("Listening on http://{}{}", addr, base_path);

    let listener = TcpListener::bind(addr).await?;
    axum::serve(listener, app.into_make_service()).await?;

    Ok(())
}

// async fn uploadcontent(
//     State(arc_state): State<Arc<RwLock<AppState>>>,
//     multipart: Multipart,
// ) -> StatusCode {

// }

async fn upload(
    State(arc_state): State<Arc<RwLock<AppState>>>,
    multipart: Multipart,
) -> StatusCode {
    let state = arc_state.read().await;
    let tcp_tx = state.tcp_tx.clone();

    match send_multipart_over_broadcast(multipart, tcp_tx).await {
        Ok(_) => StatusCode::OK,
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR,
    }
}
//SrcAndDest
async fn migrate(
    State(arc_state): State<Arc<RwLock<AppState>>>,
    Json(request): Json<SrcAndDest>,
) -> impl IntoResponse {
    let state = arc_state.read().await;

    //println!("{:#?}", request.clone());
    // &state.tcp_tx {
    match serde_json::to_vec(&request) {
        Ok(bytes) => {
            if let Err(err) = state.tcp_tx.send(bytes) {
                eprintln!("Failed to send request over broadcast: {}", err);
            }
        }
        Err(err) => eprintln!("Failed to serialize request: {}", err),
    }
    //}

    "ok"
}

// TODO: maybe split this function and route into several routes with statuses for diffrent states/nodes/settings?
async fn get_status(
    State(arc_state): State<Arc<RwLock<AppState>>>,
    Json(request): Json<IncomingMessage>,
) -> impl IntoResponse {
    let state = arc_state.write().await;

    let mut message = IncomingMessage {
        message: String::new(),
        message_type: "status".to_string(),
        authcode: "0".to_string(),
    };

    if request.message_type == "buttons" {
        match state.database.toggle_button_state().await {
            Ok(status) => {
                message.message = status.to_string();
            }
            Err(_) => {
                message.message = "error".to_string();
            }
        }
        Json(message)
    } else if request.message_type == "node" {
        Json(message)
    } else {
        Json(message)
    }
}

async fn get_buttons(State(state): State<Arc<RwLock<AppState>>>) -> impl IntoResponse {
    let mut button_list = vec![];
    match state.read().await.database.fetch_all_buttons().await {
        Ok(buttons) => {
            button_list.extend(buttons);
        }
        Err(err) => eprintln!("Error fetching DB buttons: {}", err),
    }
    Json(List {
        list: ApiCalls::ButtonList(button_list),
    })
}

async fn edit_buttons(
    State(arc_state): State<Arc<RwLock<AppState>>>,
    Json(request): Json<ModifyElementData>,
) -> impl IntoResponse {
    //println!("Got request");
    let state = arc_state.write().await;
    let result = state
        .database
        .edit_button_in_db(request)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR);
    //println!("Got result");
    result
}
async fn button_reset(
    State(arc_state): State<Arc<RwLock<AppState>>>,
    Json(request): Json<IncomingMessage>,
) -> impl IntoResponse {
    let state = arc_state.write().await;
    if request.message == "toggle" {
        let result = state.database.toggle_default_buttons().await;
        if result.is_ok() {
            StatusCode::CREATED
        } else {
            StatusCode::INTERNAL_SERVER_ERROR
        }
    } else if request.message == "restore" {
        let result = state.database.reset_buttons().await;
        if result.is_ok() {
            StatusCode::CREATED
        } else {
            StatusCode::INTERNAL_SERVER_ERROR
        }
    } else {
        StatusCode::INTERNAL_SERVER_ERROR
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct ConsoleData {
    authcode: String, 
    data: String, 
    r#type: String
}

async fn handle_socket(socket: WebSocket, arc_state: Arc<RwLock<AppState>>) {
    // Acquire lock just to get needed data
    let conn_id = {
        let state = arc_state.read().await;
        CONNECTION_COUNTER.fetch_add(1, Ordering::SeqCst)
    };

    println!("[Conn {}] NEW WEBSOCKET CONNECTION", conn_id);

    let (sender, mut receiver) = socket.split();
    let sender = Arc::new(Mutex::new(sender));

    // Subscribe to broadcast channel (no locking needed here)
    let broadcast_rx = {
        let state = arc_state.read().await;
        state.ws_tx.subscribe()
    };

    let broadcast_sender = sender.clone();

    // Spawn task forwarding broadcast messages to this client
    tokio::spawn(async move {
        let mut broadcast_rx = broadcast_rx;
        while let Ok(mut msg) = broadcast_rx.recv().await {
            // msg is a String here
            // println!("[Conn {}] Forwarding raw message: {}", conn_id, msg);

            // Try to parse msg as JSON Value
            if let Ok(mut v) = serde_json::from_str::<serde_json::Value>(&msg) {
                // println!("[Conn {}] Forwarding raw message: {:#?}", conn_id, v);
                if let Some(data_val) = v.get_mut("data") {
                    if data_val.is_array() {
                        let arr = data_val.as_array().unwrap();
                        if arr.iter().all(|item| item.is_u64()) {
                            let bytes: Vec<u8> = arr
                                .iter()
                                .map(|item| item.as_u64().unwrap() as u8)
                                .collect();

                            if let Ok(decoded_str) = std::str::from_utf8(&bytes) {
                                if let Ok(decoded_json) =
                                    serde_json::from_str::<serde_json::Value>(decoded_str)
                                {
                                    *data_val = decoded_json;

                                    if let Ok(new_msg) = serde_json::to_string(&v) {
                                        println!(
                                            "[Conn {}] Forwarding raw message: {:#?}",
                                            conn_id, new_msg
                                        );
                                        msg = new_msg;
                                    }
                                }
                            }
                        }
                    }
                }
            }

            let mut sender = broadcast_sender.lock().await;
            if sender.send(Message::Text(msg.into())).await.is_err() {
                break;
            }
        }
    });

    // Main receive loop
    while let Some(Ok(message)) = receiver.next().await {
        if let Message::Text(text) = message {
            println!("[Conn {}] Got from client: {}", conn_id, text);
            let payload = serde_json::from_str::<MessagePayload>(&text).unwrap_or(MessagePayload {
                r#type: "console".into(),
                message: text.to_string(),
                authcode: "0".into(),
            });

            if let Ok(mut bytes) = serde_json::to_vec(&payload) {
                bytes.push(b'\n');

                // Acquire lock briefly only to send TCP message
                let tcp_tx = {
                    let state = arc_state.read().await;
                    state.tcp_tx.clone()
                };

                // let mut lock = tcp_tx.lock().await;
                let mut lock = tcp_tx;
                let _ = lock.send(bytes);
            }
        }
    }

    println!("[Conn {}] DISCONNECTED", conn_id);
}


async fn ws_handler(
    ws: WebSocketUpgrade,
    State(arc_state): State<Arc<RwLock<AppState>>>,
    headers: HeaderMap,
) -> impl IntoResponse {
    // let state = arc_state.write().await;
    ws.max_frame_size(1024 * 1024)
        .max_message_size(1024 * 1024)
        .on_failed_upgrade(|e| {
            println!("WEBSOCKET UPGRADE FAILED: {:?}", e);
        })
        .on_upgrade(move |socket| {
            println!("WEBSOCKET UPGRADE SUCCESSFUL");
            handle_socket(socket, arc_state)
        })
}

// routes_static provides middlewares for authentication as well as serving all the user-orintated content
fn routes_static(state: Arc<RwLock<AppState>>) -> Router<Arc<RwLock<AppState>>> {
    let session_store = MemoryStore::default();
    let session_layer = SessionManagerLayer::new(session_store);
    let backend = Backend::default();
    let auth_layer = AuthManagerLayerBuilder::new(backend, session_layer).build();

    let base_path = std::env::var("SITE_URL")
        .map(|mut s| {
            s = s.trim().to_string();
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

    let login_url_base = Arc::new(format!("{}/index.html", base_path));

    let login_required_middleware = from_fn(
        move |auth_session: AuthSession, req: Request<Body>, next: Next| {
            let login_url_base = login_url_base.clone();
            async move {
                if auth_session.user.is_some() {
                    next.run(req).await
                } else {
                    let original_uri = req.uri();
                    let next_path = original_uri.to_string();
                    let redirect_url = format!(
                        "{}?next={}",
                        login_url_base,
                        urlencoding::encode(&next_path)
                    );
                    Redirect::temporary(&redirect_url).into_response()
                }
            }
        },
    );

    let public = Router::new()
        .route("/", get(handle_static_request))
        .route("/authenticate", get(authenticate_route))
        .route("/index.html", get(handle_static_request))
        .with_state(state.clone());

    let protected = Router::new()
        .route("/{*wildcard}", get(handle_static_request))
        .with_state(state.clone())
        .layer(login_required_middleware);

    // Apply auth_layer only once at the root
    Router::new()
        .merge(public)
        .merge(protected)
        .layer(auth_layer)
}

// async fn status(state: &AppState) -> String {
//     state.status
// }
async fn ongoing_server_status(
    State(arc_state): State<Arc<RwLock<AppState>>>,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    let interval = interval(Duration::from_secs(3));
    let state_clone = arc_state.clone();

    let updates = stream::unfold(
        (interval, state_clone),
        move |(mut interval, arc_state)| async move {
            interval.tick().await;

            let status = arc_state.read().await.status.clone();
            let status_str = match status {
                Status::Up => "up",
                Status::Healthy => "healthy",
                Status::Down => "down",
                Status::Unhealthy => "unhealthy",
            };

            Some((Ok(Event::default().data(status_str)), (interval, arc_state)))
        },
    );

    Sse::new(updates).keep_alive(axum::response::sse::KeepAlive::default())
}

async fn add_node(
    State(arc_state): State<Arc<RwLock<AppState>>>,
    Json(request): Json<ModifyElementData>,
) -> impl IntoResponse {
    let state = arc_state.write().await;
    let result = state
        .database
        .create_nodes_in_db(request)
        .await
        .map_err(|e| StatusCode::INTERNAL_SERVER_ERROR);
    result
}

// delegate user creation to the DB and return with relevent status code
async fn create_user(
    State(arc_state): State<Arc<RwLock<AppState>>>,
    Json(request): Json<ModifyElementData>,
) -> impl IntoResponse {
    let state = arc_state.write().await;
    let result = state
        .database
        .create_user_in_db(request)
        .await
        .map_err(|e| StatusCode::INTERNAL_SERVER_ERROR);
    result
}
// edits the user data in the db
async fn edit_user(
    State(arc_state): State<Arc<RwLock<AppState>>>,
    Json(request): Json<ModifyElementData>,
) -> impl IntoResponse {
    let state = arc_state.write().await;
    let result = state
        .database
        .edit_user_in_db(request)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR);
    result
}
async fn get_server(
    State(arc_state): State<Arc<RwLock<AppState>>>,
    Json(request): Json<RetrieveUser>,
) -> impl IntoResponse {
    Json({})
}

// get the user from the db
async fn get_user(
    State(arc_state): State<Arc<RwLock<AppState>>>,
    Json(request): Json<RetrieveUser>,
) -> impl IntoResponse {
    let state = arc_state.write().await;
    let result = state
        .database
        .get_from_database(&request.user)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
        .unwrap();
    Json(result)
}

// delegate user delection to the DB and returns with relevent status code
async fn delete_user(
    State(arc_state): State<Arc<RwLock<AppState>>>,
    Json(request): Json<ModifyElementData>,
) -> impl IntoResponse {
    let state = arc_state.write().await;
    let result = state
        .database
        .remove_user_in_db(request)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR);
    result
}

// TODO: (capabilities), as I am considering if the frontend needs to be notified of capabilities as it might change via featureflag
async fn capabilities(State(_): State<AppState>) -> impl IntoResponse {}

// For general messages, in alot if not most cases this is for development purposes
// there are many cases where this can fail, if it does, it can simply return INTERNAL_SERVER_ERROR
// it forwards the messages to the channel which forwards it to the gameserver
async fn process_general(
    State(arc_state): State<Arc<RwLock<AppState>>>,
    Json(res): Json<ApiCalls>,
) -> Result<Json<ResponseMessage>, (StatusCode, String)> {
    let state = arc_state.write().await;
    if let ApiCalls::IncomingMessage(payload) = res {
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
                let tx_guard = tx;

                match tx_guard.send(json_bytes) {
                    Ok(_) => {
                        println!("Successfully forwarded message to TCP server");
                        Ok(Json(ResponseMessage {
                            response: format!("Processed: {}", payload.message),
                        }))
                    }
                    Err(e) => {
                        eprintln!("Failed to send message to TCP channel: {}", e);
                        Err((
                            StatusCode::INTERNAL_SERVER_ERROR,
                            "Failed to forward message to server".to_string(),
                        ))
                    }
                }
            }
            Err(e) => {
                eprintln!("Serialization error: {}", e);
                Err((
                    StatusCode::BAD_REQUEST,
                    "Invalid message format".to_string(),
                ))
            }
        }
    } else {
        Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            "Failed to forward message to server".to_string(),
        ))
    }
}

// a list of users is returned, like alot of other routes, I need to add permissions, and check against those permissions to see if a user
// can see all the other users, it will delegate the retrival to the database and pass it in as a ApiCalls
async fn users(
    State(arc_state): State<Arc<RwLock<AppState>>>,
) -> Result<impl IntoResponse, StatusCode> {
    let state = arc_state.write().await;
    let users = state
        .database
        .fetch_all()
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(List {
        list: ApiCalls::UserList(users),
    }))
}

async fn fetch_node(
    State(arc_state): State<Arc<RwLock<AppState>>>,
    Json(request): Json<IncomingMessage>,
) -> Result<Json<Node>, StatusCode> {
    let mut state = arc_state.write().await;
    let option_node = state.database.retrieve_nodes(request.message).await;
    if let Some(node) = option_node { 
        Ok(Json(node))
    } else {
        Err(StatusCode::INTERNAL_SERVER_ERROR)
    }
}
// A list of nodes in a k8s cluster is returned, nothing is returned if there is not a client (k8s support is off)
async fn get_nodes(State(arc_state): State<Arc<RwLock<AppState>>>) -> impl IntoResponse {
    let mut state = arc_state.write().await;
    let mut node_list: Vec<NodeAndTCP> = vec![];

    if let Some(client) = state.client.clone() {
        match kubernetes::list_node_info(client).await {
            Ok(nodes) => {
                node_list.extend(nodes.clone());
            }
            Err(err) => {
                eprintln!("Error listing nodes: {}", err);
            }
        }
    }

    match state.database.fetch_all_nodes().await {
        Ok(nodes) => {
            for node in nodes {
                let new_node = NodeAndTCP {
                    name: node.nodename,
                    ip: node.ip,
                    nodetype: node.nodetype,
                    tcp_tx: None,
                    tcp_rx: None,
                };

                let exists = node_list.iter().any(|n| n.name == new_node.name);

                if !exists {
                    node_list.push(new_node);
                }
            }
        }
        Err(err) => eprintln!("Error fetching DB nodes: {}", err),
    }

    for node in node_list.clone() {
        let exists = state.additonal_node_tcp.iter().any(|n| n.name == node.name);
        if !exists {
            state.additonal_node_tcp.push(node);
        }
    }

    let node_name_list: Vec<String> = node_list.into_iter().map(|node| node.name).collect();

    Json(List {
        list: ApiCalls::NodeList(node_name_list),
    })
}

async fn get_servers(State(arc_state): State<Arc<RwLock<AppState>>>) -> impl IntoResponse {
    Json(List {
        list: ApiCalls::None,
    })
}
// TODO:, REMOVE THIS
async fn receive_message(
    State(arc_state): State<Arc<RwLock<AppState>>>,
    Json(res): Json<ApiCalls>,
) -> Result<Json<ResponseMessage>, (StatusCode, String)> {
    let state = arc_state.write().await;
    if let ApiCalls::IncomingMessage(payload) = res {
        let json_payload = MessagePayload {
            r#type: payload.message_type.clone(),
            message: payload.message.clone(),
            authcode: payload.authcode.clone(),
        };

        match serde_json::to_vec(&json_payload) {
            Ok(mut json_bytes) => {
                json_bytes.push(b'\n');

                let tx_guard = &state.tcp_tx;
                match tx_guard.send(json_bytes) {
                    Ok(_) => Ok(Json(ResponseMessage {
                        response: format!("Successfully sent message: {}", payload.message),
                    })),
                    Err(e) => {
                        eprintln!("Failed to send message to TCP channel: {}", e);
                        Err((
                            StatusCode::INTERNAL_SERVER_ERROR,
                            "Failed to forward message to server".to_string(),
                        ))
                    }
                }
            }
            Err(e) => {
                eprintln!("Serialization error: {}", e);
                Err((
                    StatusCode::BAD_REQUEST,
                    "Invalid message format".to_string(),
                ))
            }
        }
    } else {
        Err((
            StatusCode::BAD_REQUEST,
            "Invalid message format".to_string(),
        ))
    }
}

// Creates a new type for authsession with our custom backend
pub type AuthSession = axum_login::AuthSession<Backend>;

// Claims are important for JWT, particularially for expirery
#[derive(Deserialize, Serialize, Clone)]
pub struct Claims {
    pub exp: usize,
    pub iat: usize,
    pub user: String,
}

// Our custom backend, which only hash a list of users
#[derive(Clone, Default)]
pub struct Backend {
    pub users: HashMap<String, User>,
}

// Impliment the AuthBackend trait provided by axum_login for Backend, so it knows how to use it to authenticate and get users
#[async_trait]
impl AuthnBackend for Backend {
    type User = User;
    type Credentials = String;
    type Error = Infallible;

    async fn authenticate(
        &self,
        token: Self::Credentials,
    ) -> Result<Option<Self::User>, Self::Error> {
        let user = resolve_jwt(&token).ok().map(|data| User {
            username: data.claims.user,
            password_hash: None,
            user_perms: vec![],
        });
        Ok(user)
    }

    async fn get_user(&self, user_id: &String) -> Result<Option<Self::User>, Self::Error> {
        Ok(Some(User {
            username: user_id.clone(),
            password_hash: None,
            user_perms: vec![],
        }))
    }
}

// Using the secret which MUST be set, it will attempt to decode the claim, which means that it if fails to decode it, its not authorized and did not come from the secret
// and thus is unauthorized
fn resolve_jwt(token: &str) -> Result<TokenData<Claims>, StatusCode> {
    let secret = std::env::var("SECRET").unwrap_or_else(|_| {
        panic!("Need to specify a secret");
    });
    decode::<Claims>(
        token,
        &DecodingKey::from_secret(secret.as_bytes()),
        &Validation::default(),
    )
    .map_err(|_| StatusCode::UNAUTHORIZED)
}

// Creates a claim with respect to the secret, and gives it a expirery
fn encode_token(user: String) -> Result<String, StatusCode> {
    let now = Utc::now();
    let exp = (now + chrono::Duration::hours(24)).timestamp() as usize;
    let iat = now.timestamp() as usize;
    let claims = Claims { exp, iat, user };

    let secret = std::env::var("SECRET").unwrap_or_else(|_| {
        panic!("Need to specify a secret");
    });

    encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(secret.as_bytes()),
    )
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

// LoginData arrives as just a user and password
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct LoginData {
    pub user: String,
    pub password: String,
}

// When a JWT arrives from the frontend, it simply arrives with the token argument
#[derive(Deserialize)]
pub struct JwtTokenForm {
    pub token: String,
}

// Impliment AuthUser for User for axum login so it knows how to identify the user and get the hash
impl AuthUser for User {
    type Id = String;

    fn id(&self) -> Self::Id {
        self.username.clone()
    }

    fn session_auth_hash(&self) -> &[u8] {
        self.username.as_bytes()
    }
}

// The sign in function which is the main part of authentication
// rely on the database to try and find the user entry, if it fails, its immediately unauthorized, or it will try and match the password next
// if it fails, its unauthorized
#[axum::debug_handler]
pub async fn sign_in(
    State(arc_state): State<Arc<RwLock<AppState>>>,
    Form(request): Form<LoginData>,
) -> Result<Json<ResponseMessage>, StatusCode> {
    let state = arc_state.write().await;
    let user = state
        .database
        .retrieve_user(request.user.clone())
        .await
        .ok_or(StatusCode::UNAUTHORIZED)?;
    let password_valid = verify_password(request.password, user.password_hash.unwrap())
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    if !password_valid {
        return Err(StatusCode::UNAUTHORIZED);
    }

    let token = encode_token(user.username)?;
    Ok(Json(ResponseMessage { response: token }))
}

// Simple way to check if the passwords correct with bycrypt, considering the hash and normal password
pub fn verify_password(password: String, hash: String) -> Result<bool, bcrypt::BcryptError> {
    bcrypt::verify(password, &hash)
}

// We replace [[SITE_URL]], which is crucial for support with a custom prefix for routes, like so /gameserver-rs/index.html instead of just index.html,
// this is because within my HTML, I made it so by replacing the contents of a metatag with that string, the scripts read from the metatag, and some of the HREFS, and adds it as a prefix
// it also serves it with the correct mime type (content_type)
async fn serve_html_with_replacement(
    file: &str,
    state: &AppState,
) -> Result<Response<Body>, StatusCode> {
    let path = Path::new("src/svelte/build").join(file);

    if path.extension().and_then(|e| e.to_str()) == Some("html") {
        let html = tokio_fs::read_to_string(&path)
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        let replaced = html.replace("[[SITE_URL]]", &state.base_path);
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

// Handles the requests to the backend, like for static assets and so on
// this ensures that by default things are redirected to index.html, otherwised passed on normally
// and served, if its html, it will serve it with its replacement
async fn handle_static_request(
    State(arc_state): State<Arc<RwLock<AppState>>>,
    req: Request<Body>,
) -> Result<Response<Body>, StatusCode> {
    let state = arc_state.read().await;
    let path = req.uri().path();

    // let query = req.uri().query().unwrap_or("");
    // let params: FileParams = match serde_urlencoded::from_str(query) {
    //     Ok(p) => p,
    //     Err(_) => return Err(StatusCode::BAD_REQUEST),
    // };
    // let response = get_files(
    //     State(arc_state.clone()),
    //     Query(params),
    // )
    // .await
    // .into_response();

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
async fn get_files_content(
    State(state): State<Arc<RwLock<AppState>>>,
    Json(request): Json<FileChunk>,
) -> Result<impl IntoResponse, impl IntoResponse> {
    let (tcp_tx, tcp_rx) = {
        let state = state.read().await;
        (state.tcp_tx.clone(), state.tcp_tx.subscribe())
    };

    let mut tcp_fs = TcpFs::new(tcp_tx, tcp_rx);
    let mut base_path = RemoteFileSystem::new("server", Some(tcp_fs.clone()));

    let user_input = request.file_name.trim_start_matches('/');

    let requested_path = base_path
        .join(user_input)
        .canonicalize()
        .await
        .map_err(|e| {
            eprintln!("[get_files] Invalid path: {}", e);
            (StatusCode::BAD_REQUEST, "Invalid path").into_response()
        })?;

    let (dir_path, file_name) = match (requested_path.parent(), requested_path.file_name()) {
        (Some(dir), Some(file)) => (dir.to_path_buf(), file.to_os_string()),
        _ => {
            eprintln!("[get_files] Could not split path into directory and file");
            return Err((StatusCode::BAD_REQUEST, "Invalid path structure").into_response());
        }
    };

    let full_path = dir_path.join(&file_name);
    let file_chunk = FileChunk {
        file_name: full_path.to_string_lossy().to_string(),
        file_chunk_offet: request.file_chunk_offet,
        file_chunk_size: request.file_chunk_size,
    };

    let content = tcp_fs
        .get_files_content(file_chunk)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, "Failed to read file").into_response())?;

    Ok(Json(content))
}

pub async fn get_files(
    State(state): State<Arc<RwLock<AppState>>>,
    Json(request): Json<IncomingMessage>,
) -> impl IntoResponse {
    let (tcp_tx, tcp_rx) = {
        let state = state.read().await;
        (state.tcp_tx.clone(), state.tcp_tx.subscribe())
    };
    let tcp_fs = TcpFs::new(tcp_tx, tcp_rx);

    let user_input = request.message.trim_start_matches('/');

    let fs_path = if user_input.is_empty() {
        "server".to_string()
    } else if user_input == "server" {
        "server".to_string()
    } else if user_input.starts_with("server/") {
        user_input.to_string()
    } else {
        format!("server/{}", user_input)
    };

    let mut requested_path = match timeout(
        Duration::from_secs(5),
        RemoteFileSystem::new(&fs_path, Some(tcp_fs)).canonicalize(),
    )
    .await
    {
        Ok(Ok(p)) => p,
        Ok(Err(e)) => {
            eprintln!("[get_files] Invalid path '{}': {}", fs_path, e);
            return (StatusCode::BAD_REQUEST, "Invalid path").into_response();
        }
        Err(_) => {
            eprintln!("[get_files] Timeout resolving '{}'", fs_path);
            return (StatusCode::GATEWAY_TIMEOUT, "Request timed out").into_response();
        }
    };

    let is_dir = match timeout(Duration::from_secs(3), requested_path.is_dir()).await {
        Ok(Ok(true)) => true,
        Ok(Ok(false)) => {
            eprintln!(
                "[get_files] Path is not a directory: '{}'",
                requested_path.to_string()
            );
            return (StatusCode::BAD_REQUEST, "Path is not a directory").into_response();
        }
        Ok(Err(e)) => {
            eprintln!(
                "[get_files] Error checking dir '{}': {}",
                requested_path.to_string(),
                e
            );
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to check directory",
            )
                .into_response();
        }
        Err(_) => {
            eprintln!(
                "[get_files] Timeout checking dir '{}'",
                requested_path.to_string()
            );
            return (StatusCode::GATEWAY_TIMEOUT, "Request timed out").into_response();
        }
    };

    let entries = match timeout(Duration::from_secs(20), requested_path.read_dir()).await {
        Ok(Ok(list)) => list,
        Ok(Err(e)) => {
            eprintln!(
                "[get_files] Failed to read directory '{}': {}",
                requested_path.to_string(),
                e
            );
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to read directory",
            )
                .into_response();
        }
        Err(_) => {
            eprintln!(
                "[get_files] Timeout reading dir '{}'",
                requested_path.to_string()
            );
            return (StatusCode::GATEWAY_TIMEOUT, "Request timed out").into_response();
        }
    };

    let mut items = Vec::with_capacity(entries.len());
    for mut entry in entries {
        if let Some(entry_name) = entry.file_name().unwrap().to_str() {
            if let Ok(is_dir) = entry.is_dir().await {
                items.push(if is_dir {
                    FsItem::Folder(entry_name.to_string())
                } else {
                    FsItem::File(entry_name.to_string())
                });
            }
        }
    }

    Json(List {
        list: ApiCalls::FileList(items),
    })
    .into_response()
}


// This is crucial for authentication, it will take a next for redirects, and a jwk to verify the claim with, then it grants the claim for the current session
// and redirects the user to their original destination
#[derive(Deserialize)]
pub struct AuthenticateParams {
    next: String,
    jwk: String,
}

async fn authenticate_route(
    State(_state): State<Arc<RwLock<AppState>>>,
    Query(params): Query<AuthenticateParams>,
    mut auth_session: AuthSession,
) -> impl IntoResponse {
    match resolve_jwt(&params.jwk) {
        Ok(token_data) => {
            let user = User {
                username: token_data.claims.user,
                password_hash: None,
                user_perms: vec![],
            };

            if let Err(e) = auth_session.login(&user).await {
                eprintln!("Failed to log in user: {:?}", e);
                return (StatusCode::INTERNAL_SERVER_ERROR, "Failed to log in").into_response();
            }

            if params.next.starts_with('/') {
                if let Ok(uri) = params.next.parse::<Uri>() {
                    return Redirect::to(&uri.to_string()).into_response();
                } else {
                    return (
                        StatusCode::BAD_REQUEST,
                        "Invalid next parameter: unable to parse URI",
                    )
                        .into_response();
                }
            } else {
                return (
                    StatusCode::BAD_REQUEST,
                    "Invalid next parameter: must start with '/'",
                )
                    .into_response();
            }
        }
        Err(_) => (StatusCode::UNAUTHORIZED, "Invalid token").into_response(),
    }
}

// Gets a message from the server
// TODO: think about removing this
async fn get_message(
    State(arc_state): State<Arc<RwLock<AppState>>>,
) -> Result<Json<MessagePayload>, (StatusCode, String)> {
    let mut state = arc_state.write().await;
    let request = MessagePayload {
        r#type: "request".to_string(),
        message: "get_message".to_string(),
        authcode: "0".to_owned(),
    };

    let mut json_bytes = match serde_json::to_vec(&request) {
        Ok(mut v) => {
            v.push(b'\n');
            v
        }
        Err(e) => {
            eprintln!("Serialization error: {}", e);
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to serialize request".into(),
            ));
        }
    };

    let tx_guard = &state.tcp_tx;
    if let Err(e) = tx_guard.send(json_bytes) {
        eprintln!("Failed to send request: {}", e);
        return Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            "Failed to send request to server".into(),
        ));
    }
    drop(tx_guard);

    let mut rx_guard = &mut state.tcp_rx;
    match rx_guard.recv().await {
        Ok(response_bytes) => match serde_json::from_slice::<MessagePayload>(&response_bytes) {
            Ok(msg) => Ok(Json(msg)),
            Err(e) => {
                eprintln!("Deserialization error: {}", e);
                Err((
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Failed to parse server response".into(),
                ))
            }
        },
        Err(e) => {
            eprintln!("Failed to receive from broadcast: {e}");
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                "No response from server".into(),
            ))
        }
    }
}

// Unit tests
#[cfg(test)]
mod tests {
    use super::*;

    mod db_users {
        use super::*;
        use crate::database::{Database, Element};

        #[cfg(all(not(feature = "full-stack"), not(feature = "docker"), not(feature = "database")))]
        async fn create_db_for_tests() -> Result<Database, String> {
            Ok(Database::new(None))
        }

        #[cfg(any(feature = "full-stack", feature = "docker", feature = "database"))]
        async fn create_db_for_tests() -> Result<Database, sqlx::Error> {
            let conn = first_connection().await?;
            let database = database::Database::new(Some(conn));
            Ok(database)
        }

        #[tokio::test]
        #[serial]
        async fn remove_user() {
            let database = create_db_for_tests().await.unwrap();
            database.clear_db().await.expect("Failed to clear DB");
            let user = ModifyElementData {
                element: Element::User {
                    user: "kk".to_owned(),
                    password: "ddd".to_owned(),
                    user_perms: vec![],
                },
                require_auth: true,
                jwt: "".to_owned(),
            };
            let _ = database
                .create_user_in_db(user)
                .await
                .expect("Failed to clear DB");
            let remove_user_result = database
                .remove_user_in_db(ModifyElementData {
                    element: Element::User {
                        user: "kk".to_owned(),
                        password: "ddd".to_owned(),
                        user_perms: vec![], 
                    },
                    jwt: "".to_string(),
                    require_auth: false
                })
                .await;
            if remove_user_result.is_ok() {
                assert!(true)
            } else {
                assert!(false)
            }
        }

        #[tokio::test]
        #[serial]
        async fn create_user_perms() {
            let database = create_db_for_tests().await.unwrap();
            database.clear_db().await.expect("Failed to clear DB");
            let user = ModifyElementData {
                element: Element::User {
                    user: "kk".to_owned(),
                    password: "ddd".to_owned(),
                    user_perms: vec!["test".to_string()],
                },
                require_auth: true,
                jwt: "".to_owned(),
            };
            let _ = database.create_user_in_db(user).await;
            let retrieved_user_option = database.retrieve_user("kk".to_string()).await;
            if let Some(retrieved_user) = retrieved_user_option {
                assert_eq!(retrieved_user.user_perms, vec!["test"])
            } else {
                assert!(false)
            }
        }

        #[tokio::test]
        #[serial]
        async fn create_user() {
            let database = create_db_for_tests().await.unwrap();
            database.clear_db().await.expect("Failed to clear DB");
            let user = ModifyElementData {
                element: Element::User {
                    user: "kk".to_owned(),
                    password: "ddd".to_owned(),
                    user_perms: vec![],
                },
                require_auth: true,
                jwt: "".to_owned(),
            };
            let create_user_result = database.create_user_in_db(user).await;
            if create_user_result.is_ok() {
                assert!(true)
            } else {
                assert!(false)
            }
        }

        #[tokio::test]
        #[serial]
        async fn edit_user_password_changes() {
            let database = create_db_for_tests().await.unwrap();
            database.clear_db().await.expect("Failed to clear DB");
            let user = ModifyElementData {
                element: Element::User {
                    user: "b".to_owned(),
                    password: "ddd".to_owned(),
                    user_perms: vec![],
                },
                require_auth: true,
                jwt: "".to_owned(),
            };
            let create_user_result = database.create_user_in_db(user).await;

            let edit_user = ModifyElementData {
                element: Element::User {
                    user: "b".to_owned(),
                    password: "ccc".to_owned(),
                    user_perms: vec![],
                },
                require_auth: true,
                jwt: "".to_owned(),
            };
            let result = database.edit_user_in_db(edit_user).await;
            let final_user = database.retrieve_user("b".to_string()).await;
            if let Some(user) = final_user {
                assert!(bcrypt::verify("ccc", &user.password_hash.unwrap()).is_ok())
            } else {
                assert!(false)
            }
        }
        #[tokio::test]
        #[serial]
        async fn edit_user_password_does_not_change() {
            let database = create_db_for_tests().await.unwrap();
            database.clear_db().await.expect("Failed to clear DB");
            let user = ModifyElementData {
                element: Element::User {
                    user: "A".to_owned(),
                    password: "a".to_owned(),
                    user_perms: vec![],
                },
                require_auth: true,
                jwt: "".to_owned(),
            };
            let create_user_result = database.create_user_in_db(user).await;
            let edit_user = ModifyElementData {
                element: Element::User {
                    user: "A".to_owned(),
                    password: "".to_owned(),
                    user_perms: vec![],
                },
                require_auth: true,
                jwt: "".to_owned(),
            };
            let result = database.edit_user_in_db(edit_user).await;
            let final_user = database.retrieve_user("A".to_string()).await;
            if let Some(user) = final_user {
                assert!(bcrypt::verify("a", &user.password_hash.unwrap()).is_ok())
            } else {
                assert!(false)
            }
        }

        #[tokio::test]
        #[serial]
        async fn empty_password() {
            let database = create_db_for_tests().await.unwrap();
            database.clear_db().await.expect("Failed to clear DB");
            let user = ModifyElementData {
                element: Element::User {
                    user: "A".to_owned(),
                    password: "".to_owned(),
                    user_perms: vec![],
                },
                require_auth: true,
                jwt: "".to_owned(),
            };
            let result = database.create_user_in_db(user).await;
            if result.is_err() {
                assert!(true)
            } else {
                assert!(false)
            }
        }

        #[tokio::test]
        #[serial]
        async fn duplicate_user() {
            let database = create_db_for_tests().await.unwrap();
            database.clear_db().await.expect("Failed to clear DB");
            let userA = ModifyElementData {
                element: Element::User {
                    user: "A".to_owned(),
                    password: "test".to_owned(),
                    user_perms: vec![],
                },
                require_auth: true,
                jwt: "".to_owned(),
            };
            let userB = ModifyElementData {
                element: Element::User {
                    user: "A".to_owned(),
                    password: "test".to_owned(),
                    user_perms: vec![],
                },
                require_auth: true,
                jwt: "".to_owned(),
            };
            let resultA = database.create_user_in_db(userB).await;
            let resultB = database.create_user_in_db(userA).await;

            if resultB.is_err() {
                assert!(true)
            } else {
                assert!(false)
            }
        }
    }
    // Ok(())
}
