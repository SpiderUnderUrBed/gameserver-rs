use std::any::Any;
use std::cell::RefCell;
// ALOT of imports, needed given the size of this project in what it covers
// first imports are std ones
use std::collections::HashMap;
use std::convert::Infallible;
use std::ffi::OsStr;
use std::fmt::{self, Debug};
// use std::fs;
// use std::rc::Rc;
use std::borrow::BorrowMut;
use std::fs;
use std::ops::Deref;
use std::path::{Component, Components, PathBuf};
use std::{net::SocketAddr, path::Path, sync::Arc};

use tracing::{debug, error};

// Axum is the routing framework, and the backbone to this project helping intergrate the backend with the frontend
// and the general api, redirections, it will take form data and queries and make it easily accessible
// I also use axum_login to take off alot of effort that would be required for authentication
use crate::database::Element;
use crate::http::HeaderMap;
use crate::middleware::from_fn;
use axum::extract::ws::Message as WsMessage;
use axum::extract::FromRequest;
use axum::extract::Query;
use axum::http::header::AUTHORIZATION;
use axum::http::Uri;
use axum::middleware::{self, Next};
use axum::response::Redirect;
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
use axum::{Extension, Form};
use axum_login::tower_sessions::{MemoryStore, SessionManagerLayer};
use axum_login::AuthUser;
use axum_login::{login_required, AuthManagerLayerBuilder, AuthnBackend, UserId};
use futures_util::stream::once;
use serde::de::DeserializeOwned;
use tokio::sync::RwLock;

// mod databasespec;
// use databasespec::UserDatabase;
use crate::broadcast::Receiver;
use crate::database::databasespec::Button;
use crate::database::databasespec::ButtonsDatabase;
use crate::database::databasespec::NodesDatabase;
use crate::database::databasespec::UserDatabase;
use crate::database::Node;
// miscellancious imports, future traits are used because alot of the code is asyncronus and cant fully be contained in tokio
// mime_guess as when I am serving the files, I need to serve it with the correct mime type
// serde_json because I exchange alot of json data between the backend and frontend and to the gameserver
// tokio because when working with alot of networking stuff and things that will take a indeterminent amount of time, async/await is the way to go (for better efficency too)
// chrono for time, tower for cors (TODO:: use less permissive CORS due to potential security risks)
// jsonwebtokens is standard when working with authentication, and bcrypt so I can use password hashs, I explain the authentication methods later
use async_trait::async_trait;
use bcrypt::BcryptError;
use bcrypt::{hash, verify, DEFAULT_COST};
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
    sync::{broadcast, mpsc, Mutex},
    time::{timeout, Duration},
};
use tower_http::add_extension::AddExtensionLayer;
use tower_http::cors::{Any as CorsAny, CorsLayer};

use futures_util::{stream, Stream};

use std::sync::atomic::{AtomicUsize, Ordering};
use tokio_util::bytes::Bytes;
static CONNECTION_COUNTER: AtomicUsize = AtomicUsize::new(0);
use tokio::time::interval;
use tokio::time::Instant;

// For now I only restrict the json backend for running this without kubernetes
// the json backend is only for testing in most cases, simple deployments would use full-stack feature flag
// and you can use postgres manually with the database feature flag
#[cfg(any(feature = "full-stack", feature = "database"))]
mod database {
    include!("pgdatabase.rs");
}

#[cfg(all(not(feature = "full-stack"), not(feature = "database")))]
mod database {
    include!("jsondatabase.rs");
}

// JsonDatabase is only something that would be unique to Json and not any other database managed by sqlx
#[cfg(all(not(feature = "full-stack"), not(feature = "database")))]
use database::JsonBackend;

// Both database files and any more should have these structs
use database::CreateElementData;
use database::RemoveElementData;
use database::RetrieveUser;
use database::User;
// use databasespec::{User, CreateUserData, RemoveUserData};

mod filesystem;
use filesystem::{FsItem, RemoteFileSystem, TcpFs, FsEntry, FileResponseMessage, FsMetadata};

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

#[cfg(feature = "full-stack")]
static TcpUrl: &str = "gameserver-service:8080";

#[cfg(feature = "full-stack")]
static LocalUrl: &str = "127.0.0.1:8080";

static WEBSOCKET_DEBUGGING: bool = false;
// K8S_WORKS needs to be true in the case where the full stack is running and not if that is not the case
// to avoid calling the dummy functions
#[cfg(feature = "full-stack")]
static K8S_WORKS: bool = true;

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
#[cfg(any(feature = "full-stack", feature = "database"))]
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
#[cfg(all(not(feature = "full-stack"), not(feature = "database")))]
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
    // tcp_tx: Arc<Mutex<mpsc::Sender<Vec<u8>>>>,
    tcp_tx: tokio::sync::broadcast::Sender<Vec<u8>>,
    tcp_rx: tokio::sync::broadcast::Receiver<Vec<u8>>,
    // tcp_rx: Arc<Mutex<mpsc::Receiver<Vec<u8>>>>,
    gameserver: Value,
    status: Status,
    ws_tx: broadcast::Sender<String>,
    // peer_addr: Option<String>,
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
        }
    }
}
// static ACCEPT_NON_ID_MESSAGE: bool = false;

// for the initial connection attempt, which will determine if possibly I would need to create the container and deployment upon failure
// i will use rusts 'timeout' for x interval determined with CONNECTION_TIMEOUT
async fn attempt_connection() -> Result<TcpStream, Box<dyn std::error::Error + Send + Sync>> {
    timeout(CONNECTION_TIMEOUT, TcpStream::connect(TcpUrl))
        .await?
        .map_err(Into::into)
}
fn parse_json_objects_in_str<T>(input: &str) -> Vec<Result<T, serde_json::Error>>
where
    T: DeserializeOwned + Debug,
{
    let mut results = Vec::new();
    let mut remaining = input;

    while let Some(start) = remaining.find('{') {
        let mut open_braces = 0usize;
        let mut end_index = None;

        for (i, c) in remaining[start..].chars().enumerate() {
            if c == '{' {
                open_braces += 1;
            } else if c == '}' {
                open_braces -= 1;
                if open_braces == 0 {
                    end_index = Some(start + i + 1);
                    break;
                }
            }
        }

        if let Some(end) = end_index {
            let candidate = &remaining[start..end];
            match serde_json::from_str::<Value>(candidate) {
                Ok(val) => {
                    if let Ok(inner_data) = serde_json::from_value::<InnerData>(val.clone()) {
                        match serde_json::from_str::<T>(&inner_data.data) {
                            Ok(parsed) => results.push(Ok(parsed)),
                            Err(e) => results.push(Err(e)),
                        }
                    } else {
                        match serde_json::from_value::<T>(val) {
                            Ok(parsed) => results.push(Ok(parsed)),
                            Err(e) => results.push(Err(e)),
                        }
                    }
                }
                Err(e) => {
                    results.push(Err(e));
                }
            }

            remaining = &remaining[end..];
        } else {
            break;
        }
    }

    results
}

async fn value_from_line<T, F>(gameserver_str: &str, filter: F) -> Vec<Result<T, serde_json::Error>>
where
    T: DeserializeOwned + Debug,
    F: Fn(&str) -> bool,
{
    let mut final_values = vec![];

    for line in gameserver_str.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        if !filter(line) {
            continue;
        }

        match serde_json::from_str::<Value>(line) {
            Ok(line_val) => {
                if let Ok(resp_msg) =
                    serde_json::from_value::<FileResponseMessage>(line_val.clone())
                {
                    match String::from_utf8(resp_msg.data) {
                        Ok(data_str) => match serde_json::from_str::<T>(&data_str) {
                            Ok(result) => {
                                final_values.push(Ok(result));
                            }
                            Err(e) => {
                                final_values.push(Err(e));
                            }
                        },
                        Err(e) => {
                            final_values.push(Err(serde_json::from_str::<T>("").unwrap_err()));
                        }
                    }
                } else if let Ok(inner_data) = serde_json::from_value::<InnerData>(line_val.clone())
                {
                    match serde_json::from_str::<T>(&inner_data.data) {
                        Ok(result) => {
                            final_values.push(Ok(result));
                        }
                        Err(e) => {
                            final_values.push(Err(e));
                        }
                    }
                } else {
                    match serde_json::from_value::<T>(line_val) {
                        Ok(result) => {
                            final_values.push(Ok(result));
                        }
                        Err(e) => {
                            final_values.push(Err(e));
                        }
                    }
                }
            }
            Err(e) => {
                println!("Failed to parse line as JSON: {}", e);
                let partials = parse_json_objects_in_str::<T>(line);
                if partials.is_empty() {
                    println!(
                        "Failed to parse line as JSON and no valid JSON objects found inside."
                    );
                }
                final_values.extend(partials);
            }
        }
    }

    final_values
}
async fn handle_server_data(
    state: Arc<RwLock<AppState>>,
    data: &[u8],
    ws_tx: &broadcast::Sender<String>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let text = String::from_utf8_lossy(data);

    let bytes: Vec<u8> = text
        .split(|c: char| !c.is_ascii_digit())
        .filter_map(|s| {
            s.parse::<u64>()
                .ok()
                .and_then(|n| if n <= 255 { Some(n as u8) } else { None })
        })
        .collect();

    if let Ok(resp_msg) = serde_json::from_str::<FileResponseMessage>(&text) {
        let state_guard = state.read().await;
        if let Err(e) = state_guard.tcp_tx.send(serde_json::to_vec(&resp_msg)?) {
            println!("Failed to forward FileResponseMessage: {}", e);
        }
        return Ok(());
    }

    if let Ok(json_value) = serde_json::from_str::<serde_json::Value>(&text) {
        if let Some(in_response_to) = json_value.get("in_response_to") {
            let resp_msg = FileResponseMessage {
                in_response_to: in_response_to.as_u64().unwrap_or(0),
                data: if let Some(data) = json_value.get("data") {
                    serde_json::to_vec(data)?
                } else {
                    serde_json::to_vec(&json_value)?
                },
            };
            let state_guard = state.read().await;
            if let Err(e) = state_guard.tcp_tx.send(serde_json::to_vec(&resp_msg)?) {
                println!("Failed to forward wrapped response: {}", e);
            }
            return Ok(());
        }
    }

    // Handle raw metadata or entries
    if let Ok(metadata) = serde_json::from_slice::<FsMetadata>(data) {
        let resp_msg = FileResponseMessage {
            in_response_to: 0, // Assuming 0 is metadata request ID
            data: serde_json::to_vec(&metadata)?,
        };
        let state_guard = state.read().await;
        if let Err(e) = state_guard.tcp_tx.send(serde_json::to_vec(&resp_msg)?) {
            println!("Failed to forward metadata: {}", e);
        }
    } else if let Ok(entries) = serde_json::from_slice::<Vec<FsEntry>>(data) {
        let resp_msg = FileResponseMessage {
            in_response_to: 1, // Assuming 1 is list directory request ID
            data: serde_json::to_vec(&entries)?,
        };
        let state_guard = state.read().await;
        if let Err(e) = state_guard.tcp_tx.send(serde_json::to_vec(&resp_msg)?) {
            println!("Failed to forward entries: {}", e);
        }
    }

    if let Ok(outer_msg) = serde_json::from_str::<InnerData>(&text) {
        let inner_data_str = outer_msg.data.as_str();
        let mut borrowed_state = state.write().await;

        let mut keyword_detected = false;
        if let Ok(keyword_payload) = serde_json::from_str::<KeywordPayload>(inner_data_str) {
            if let Some(start_keyword) = &keyword_payload.start_keyword {
                if inner_data_str.contains(start_keyword) {
                    println!("Changed state to Up (current line)");
                    borrowed_state.status = Status::Up;
                    keyword_detected = true;
                }
            }

            if let Some(stop_keyword) = &keyword_payload.stop_keyword {
                if inner_data_str.contains(stop_keyword) {
                    println!("Changed state to Down (current line)");
                    borrowed_state.status = Status::Down;
                    keyword_detected = true;
                }
            }
        }

        let filter = |line: &str| line.contains("start_keyword") || line.contains("stop_keyword");
        if !keyword_detected {
            if let Some(gameserver_str) = borrowed_state.gameserver.as_str() {
                if let Some(last_keyword_line) = gameserver_str
                    .lines()
                    .rev()
                    .find(|l| l.contains("start_keyword") || l.contains("stop_keyword"))
                {
                    let values =
                        value_from_line::<KeywordPayload, _>(last_keyword_line, filter).await;
                    for value in values {
                        if let Ok(parsed_data) = value {
                            if let Some(start_keyword) = &parsed_data.start_keyword {
                                if inner_data_str.contains(start_keyword) {
                                    println!("Changed state to Up (from log)");
                                    borrowed_state.status = Status::Up;
                                }
                            }
                            if let Some(stop_keyword) = &parsed_data.stop_keyword {
                                if inner_data_str.contains(stop_keyword) {
                                    println!("Changed state to Down (from log)");
                                    borrowed_state.status = Status::Down;
                                }
                            }
                        }
                    }
                }
            } else {
                eprintln!("gameserver is not a string");
            }
        }

        if let Ok(inner_data) = serde_json::from_str::<serde_json::Value>(inner_data_str) {
            if ALLOW_NONJSON_DATA {
                if let Some(message_content) = inner_data["data"].as_str() {
                    println!("Extracted message: {}", message_content);
                    let _ = ws_tx.send(message_content.to_string());
                }
            } else {
                println!("Sending raw inner data: {}", inner_data_str);
                let _ = ws_tx.send(inner_data_str.to_string());
            }
        } else {
            println!("Sending raw inner data: {}", inner_data_str);
            let _ = ws_tx.send(inner_data_str.to_string());
        }
    } else {
        println!("Sending raw text: {}", text);
        let _ = ws_tx.send(text.to_string());
    }

    Ok(())
}

// this handles the main tcp stream between the gameserver and the client console, more specifically the data exchanged
// as well as notifying the gameserver of its capabilities which at some point ill make dependent on what feature flag is set
// `tokio::select!` is used to concurrently wait for either incoming TCP data or messages from the channel to send.
async fn handle_stream(
    state: Arc<RwLock<AppState>>,
    rx: &mut tokio::sync::broadcast::Receiver<Vec<u8>>,
    stream: &mut TcpStream,
    ws_tx: broadcast::Sender<String>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let ip = stream.peer_addr()?.to_string();
    let (mut reader, mut writer) = stream.split();
    let mut buf = vec![0u8; 1024];
    let mut buf_reader = BufReader::new(reader);
    let mut line = String::new();

    let capability_msg = serde_json::to_string(&List {
        list: ApiCalls::Capabilities(vec!["all".to_string()]),
    })? + "\n";

    writer.write_all(capability_msg.as_bytes()).await?;
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

    let server_data_msg = serde_json::to_string(&MessagePayload {
        r#type: "command".to_string(),
        message: "server_data".to_string(),
        authcode: "0".to_string(),
    })? + "\n";

    writer.write_all(server_data_msg.as_bytes()).await?;
    match buf_reader.read_line(&mut line).await {
        Ok(0) => {
            println!("Error, possibly connection closed");
        }
        Ok(_) => {
            println!("Received line: {}", line.trim_end());
            state.write().await.gameserver = serde_json::Value::String(line.clone());
        }
        Err(e) => {
            println!("Error reading line: {:?}", e);
            return Err(e.into());
        }
    }
    println!("State: {:#?}", state.read().await.gameserver);

    let server_node_name_msg = serde_json::to_string(&MessagePayload {
        r#type: "command".to_string(),
        message: "server_name".to_string(),
        authcode: "0".to_string(),
    })? + "\n";

    writer.write_all(server_node_name_msg.as_bytes()).await?;
    match buf_reader.read_line(&mut line).await {
        Ok(0) => {
            println!("Error, possibly connection closed");
        }
        Ok(_) => {
            println!("Received line: {}", line.trim_end());
            //let outer: ConsoleMessage = serde_json::from_str(&line)?;
            //let nodename_result = serde_json::from_str::<MessagePayload>(&outer.data);
            //if let Ok(name_struct_option) = value_from_line::<ConsoleMessage>(&line).await {
            for value in value_from_line::<ConsoleMessage, _>(&line, |_| true).await {
                if let Ok(potential_name_struct) = value {
                    if let Ok(name_struct) =
                        serde_json::from_str::<MessagePayload>(&potential_name_struct.data)
                    {
                        println!("Got the name");
                        let db_state = &state.write().await.database;

                        if let Ok(nodes) = db_state.fetch_all_nodes().await {
                            println!("{:#?}", name_struct.message);
                            println!("Got nodes");

                            let node = Node {
                                ip: ip.clone(),
                                nodename: name_struct.message,
                                nodetype: "main".to_string(),
                            };

                            if !nodes
                                .iter()
                                .any(|n| n.ip == node.ip && n.nodename == node.nodename)
                            {
                                //.contains(&node) {
                                println!("Making node");
                                let _ = db_state
                                    .create_nodes_in_db(CreateElementData {
                                        element: Element::Node(node),
                                        jwt: "".to_string(),
                                        require_auth: false,
                                    })
                                    .await;
                            }
                        }
                    } else {
                        println!("No nodes");
                    }
                } else {
                    println!("No options");
                    // optional: log error
                }
            }
            // else if let Err(errs) = value_from_line::<MessagePayload>(&line).await {
            //     for err in errs {
            //         println!("{:#?}", err);
            //     }
            // }
            //state.write().await.gameserver = serde_json::Value::String(line.clone());
        }
        Err(e) => {
            println!("Error reading line: {:?}", e);
            return Err(e.into());
        }
    }

    reader = buf_reader.into_inner();

    loop {
        let res: Result<(), Box<dyn std::error::Error + Send + Sync>> = tokio::select! {
            result = reader.read(&mut buf) => {
                match result {
                    Ok(0) => break Ok(()),
                    Ok(n) => {
                        handle_server_data(state.clone(), &buf[..n], &ws_tx).await?;
                        Ok(())
                    }
                    Err(e) => break Err(e.into()),
                }
            }
            result = rx.recv() => {
                match result {
                    Ok(data) => {
                        writer.write_all(&data).await?;
                        writer.write_all(b"\n").await?;
                        writer.flush().await?;
                        Ok(())
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(skipped)) => {
                        eprintln!("Lagged: missed {} messages", skipped);
                        Ok(())
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                        break Ok(());
                    }
                }
            }
        };
        res?;
    }
}

// does the connection to the tcp server, wether initial or not, on success it will pass it off to the dedicated handler for the stream
async fn connect_to_server(
    state: Arc<RwLock<AppState>>,
    mut rx: tokio::sync::broadcast::Receiver<Vec<u8>>,
    ws_tx: broadcast::Sender<String>,
) -> Result<SocketAddr, Box<dyn std::error::Error + Send + Sync>> {
    loop {
        match timeout(CONNECTION_TIMEOUT, TcpStream::connect(TcpUrl)).await {
            Ok(Ok(mut stream)) => {
                let stream_result =
                    handle_stream(state.clone(), &mut rx, &mut stream, ws_tx.clone()).await;
                if stream_result.is_ok() {
                    return Ok(stream.peer_addr()?);
                    // Node {
                    //     nodename: "".to_string(),
                    //     ip: stream.peer_addr(),
                    // }
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
    ws_tx: broadcast::Sender<String>,
    tcp_tx: tokio::sync::broadcast::Sender<Vec<u8>>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    match attempt_connection().await {
        Ok(mut stream) => {
            println!("Initial connection succeeded!");

            let (temp_tx, temp_rx) =
                tokio::sync::broadcast::channel::<Vec<u8>>(CHANNEL_BUFFER_SIZE);

            // {
            //     let mut guard = tcp_tx;
            //     *guard = temp_tx;
            // }

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

    // creates a websocket broadcase and tcp channels
    let (ws_tx, _) = broadcast::channel::<String>(CHANNEL_BUFFER_SIZE);
    let (tcp_tx, tcp_rx) = broadcast::channel::<Vec<u8>>(CHANNEL_BUFFER_SIZE);

    // sets the client to be none by default unless this is ran the stanard way which will be ran with the appropriate feature-flag
    // which will set the k8s client
    let mut client: Option<Client> = None;
    if ENABLE_K8S_CLIENT && K8S_WORKS {
        client = Some(Client::try_default().await?);
    }

    // use everything so far to make the app state
    let state = AppState {
        gameserver: json!({}),
        status: Status::Down,
        tcp_tx: tcp_tx,
        // tcp_rx: Arc::new(Mutex::new(tcp_rx)),
        tcp_rx: tcp_rx,
        ws_tx: ws_tx.clone(),
        base_path: base_path.clone(),
        database,
        client,
    };

    let multifaceted_state = Arc::new(RwLock::new(state));

    // if there is supposed to be a initial connection and if there is a client (as it wont be able to create the deployment without it, and it would be pointless to create a docker container
    // without the abbility to deploy it)
    if ENABLE_INITIAL_CONNECTION && multifaceted_state.write().await.client.is_some() {
        println!("Trying initial connection...");
        if try_initial_connection(
            multifaceted_state.clone(),
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
        if let Err(e) = connect_to_server(server_connection_state, bridge_rx, bridge_tx).await {
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
        .route("/api/awaitserverstatus", get(ongoing_server_status))
        .route("/api/getfiles", post(get_files))
        .route("/api/buttonreset", post(button_reset))
        .route("/api/editbuttons", post(edit_buttons))
        .route("/api/addnode", post(add_node))
        .route("/api/edituser", post(edit_user))
        .route("/api/getuser", post(get_user))
        .route("/api/send", post(receive_message))
        .route("/api/general", post(process_general))
        .route("/api/signin", post(sign_in))
        .route("/api/createuser", post(create_user))
        .route("/api/deleteuser", post(delete_user))
        .merge(fallback_router)
        .with_state(multifaceted_state.clone());
    // let inner_async = Router::new()
    //    .route("/api/ws", get(ws_handler))
    //    .with_state(Arc::new(state.clone()));

    let normal_routes = Router::new().merge(inner);
    // .merge(inner_async);

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

async fn get_buttons(State(state): State<Arc<RwLock<AppState>>>) -> impl IntoResponse {
    let mut button_list = vec![];
    match state.read().await.database.fetch_all_buttons().await {
        Ok(buttons) => {
            //println!("{:#?}", buttons);
            // let buttonname_list: Vec<String> = buttons.iter().map(|button| button.name.clone()).collect();
            // for button in buttonname_list {
            //     if !button_list.contains(&button){
            //         button_list.push(button);
            //     }
            // }
            button_list.extend(buttons);
        }
        Err(err) => eprintln!("Error fetching DB buttons: {}", err),
    }
    //println!("{:#?}", List { list: ApiCalls::ButtonList(button_list.clone()) });
    Json(List {
        list: ApiCalls::ButtonList(button_list),
    })
}

async fn edit_buttons(
    State(arc_state): State<Arc<RwLock<AppState>>>,
    Json(request): Json<CreateElementData>,
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

async fn ws_debug(
    conn_id: usize,
    arc_state: Arc<RwLock<AppState>>,
    sender: Arc<Mutex<stream::SplitSink<WebSocket, WsMessage>>>,
    receiver: &mut stream::SplitStream<WebSocket>,
) {
    let state = arc_state.write().await;
    // Ping task with more visible logging
    let ping_task = {
        let conn_id = conn_id;
        let sender = Arc::clone(&sender);
        let mut interval = interval(Duration::from_secs(30));

        tokio::spawn(async move {
            println!("[Conn {}] PING TASK STARTED", conn_id);

            loop {
                interval.tick().await;
                println!("[Conn {}] SENDING PING", conn_id); // <-- Log ping attempts

                let mut sender = sender.lock().await;
                match sender.send(Message::Ping(Bytes::new())).await {
                    Ok(_) => println!("[Conn {}] PING SENT SUCCESSFULLY", conn_id),
                    Err(e) => {
                        println!("[Conn {}] PING FAILED: {}", conn_id, e);
                        break;
                    }
                }
            }

            println!("[Conn {}] PING TASK EXITING", conn_id);
        })
    };

    // Broadcast receiver task with more visible logging
    let broadcast_task = {
        let conn_id = conn_id;
        let sender = Arc::clone(&sender);
        let mut broadcast_rx = state.ws_tx.subscribe();

        tokio::spawn(async move {
            println!("[Conn {}] BROADCAST TASK STARTED", conn_id);

            while let Ok(msg) = broadcast_rx.recv().await {
                println!("[Conn {}] RECEIVED BROADCAST: {}", conn_id, msg);

                let mut sender = sender.lock().await;
                match sender.send(Message::Text(msg.into())).await {
                    Ok(_) => println!("[Conn {}] FORWARDED MESSAGE", conn_id),
                    Err(e) => {
                        println!("[Conn {}] FAILED TO FORWARD: {}", conn_id, e);
                        break;
                    }
                }
            }

            println!("[Conn {}] BROADCAST TASK EXITING", conn_id);
        })
    };

    // Main message processing loop with more visible logging
    println!("[Conn {}] STARTING MESSAGE PROCESSING", conn_id);

    while let Some(result) = receiver.next().await {
        match result {
            Ok(Message::Text(text)) => {
                println!("[Conn {}] RECEIVED TEXT: {}", conn_id, text);

                match serde_json::from_str::<MessagePayload>(&text) {
                    Ok(payload) => {
                        println!("[Conn {}] PARSED PAYLOAD: {:?}", conn_id, payload);

                        match serde_json::to_vec(&payload) {
                            Ok(mut bytes) => {
                                bytes.push(b'\n');
                                println!("[Conn {}] SERIALIZED TO {} BYTES", conn_id, bytes.len());

                                match state.tcp_tx.send(bytes) {
                                    Ok(_) => println!("[Conn {}] SENT TO TCP", conn_id),
                                    Err(e) => println!("[Conn {}] TCP SEND FAILED: {}", conn_id, e),
                                }
                            }
                            Err(e) => println!("[Conn {}] SERIALIZATION FAILED: {}", conn_id, e),
                        }
                    }
                    Err(e) => println!("[Conn {}] PARSE FAILED: {}", conn_id, e),
                }
            }
            Ok(Message::Binary(bin)) => {
                println!("[Conn {}] RECEIVED BINARY ({} bytes)", conn_id, bin.len());
            }
            Ok(Message::Ping(data)) => {
                println!("[Conn {}] RECEIVED PING ({} bytes)", conn_id, data.len());
            }
            Ok(Message::Pong(data)) => {
                println!("[Conn {}] RECEIVED PONG ({} bytes)", conn_id, data.len());
            }
            Ok(Message::Close(frame)) => {
                println!("[Conn {}] CLOSE FRAME: {:?}", conn_id, frame);
                break;
            }
            Err(e) => {
                println!("[Conn {}] WEBSOCKET ERROR: {}", conn_id, e);
                break;
            }
        }
    }

    println!("[Conn {}] SHUTTING DOWN", conn_id);
    ping_task.abort();
    broadcast_task.abort();

    match ping_task.await {
        Ok(_) => println!("[Conn {}] PING TASK SHUT DOWN", conn_id),
        Err(e) => println!("[Conn {}] PING TASK ERROR: {:?}", conn_id, e),
    }

    match broadcast_task.await {
        Ok(_) => println!("[Conn {}] BROADCAST TASK SHUT DOWN", conn_id),
        Err(e) => println!("[Conn {}] BROADCAST TASK ERROR: {:?}", conn_id, e),
    }
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
    Json(request): Json<CreateElementData>,
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
    Json(request): Json<CreateElementData>,
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
    Json(request): Json<CreateElementData>,
) -> impl IntoResponse {
    let state = arc_state.write().await;
    let result = state
        .database
        .edit_user_in_db(request)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR);
    result
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
    Json(request): Json<RemoveElementData>,
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

// A list of nodes in a k8s cluster is returned, nothing is returned if there is not a client (k8s support is off)
async fn get_nodes(State(arc_state): State<Arc<RwLock<AppState>>>) -> impl IntoResponse {
    let state = arc_state.write().await;
    let mut node_list = vec![];
    if state.client.is_some() {
        match kubernetes::list_node_names(state.client.clone().unwrap()).await {
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
            let nodename_list: Vec<String> =
                nodes.iter().map(|node| node.nodename.clone()).collect();
            for node in nodename_list {
                if !node_list.contains(&node) {
                    node_list.push(node);
                }
            }
        }
        Err(err) => eprintln!("Error fetching DB nodes: {}", err),
    }
    Json(List {
        list: ApiCalls::NodeList(node_list),
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
pub async fn get_files(
    State(state): State<Arc<RwLock<AppState>>>,
    Json(request): Json<IncomingMessage>,
) -> impl IntoResponse {
    println!("Received get_files request: {:?}", request);

    // Acquire read lock on state
    let (tcp_tx, tcp_rx) = {
        let state = state.read().await;
        println!(
            "[get_files] Acquired state read lock. tcp_tx subscribers: {}, tcp_rx receivers: {}",
            state.tcp_tx.receiver_count(),
            // You can't easily get receiver_count from tcp_rx because it's a Receiver, so:
            // Just log that the receiver exists
            "1 (single receiver)"
        );

        // IMPORTANT: create fresh receiver from sender (not from existing receiver)
        (state.tcp_tx.clone(), state.tcp_tx.subscribe())
    };

    println!(
        "[get_files] Cloned tcp_tx with {} subscribers, created new tcp_rx subscriber",
        tcp_tx.receiver_count()
    );

    // Create TcpFs instance directly with the broadcast sender and receiver
    let tcp_fs = TcpFs::new(tcp_tx, tcp_rx);

    // Initialize RemoteFileSystem
    let mut base_path = RemoteFileSystem::new("server", Some(tcp_fs));
    println!(
        "[get_files] Initialized RemoteFileSystem at path: {}",
        base_path.to_string()
    );

    let user_input = request.message.trim_start_matches('/');
    println!("[get_files] User input after trim: '{}'", user_input);

    // Canonicalize requested path with logging
    let mut requested_path = match base_path.join(user_input).canonicalize().await {
        Ok(path) => {
            println!(
                "[get_files] Canonicalized requested path: {}",
                path.to_string()
            );
            path
        }
        Err(e) => {
            eprintln!("[get_files] Invalid path: {}", e);
            return (StatusCode::BAD_REQUEST, "Invalid path").into_response();
        }
    };

    // Security check
    // if !requested_path.to_string().starts_with(&base_path.to_string()) {
    //     eprintln!(
    //         "[get_files] Security check failed: {} not in {}",
    //         requested_path.to_string(),
    //         base_path.to_string()
    //     );
    //     return (StatusCode::FORBIDDEN, "Forbidden: escape attempt").into_response();
    // }

    // Read directory with detailed logging
    match requested_path.read_dir().await {
        Ok(entries) => {
            println!(
                "[get_files] Successfully read directory: {} with {} entries",
                requested_path.to_string(),
                entries.len()
            );

            let mut items = Vec::new();
            for mut entry in entries {
                if let Some(name) = entry.file_name() {
                    let name_str = name.to_string_lossy().into_owned();
                    if entry.is_dir().await.unwrap_or(false) {
                        println!("[get_files] Found folder: {}", name_str);
                        items.push(FsItem::Folder(name_str));
                    } else if entry.is_file().await.unwrap_or(false) {
                        println!("[get_files] Found file: {}", name_str);
                        items.push(FsItem::File(name_str));
                    } else {
                        println!("[get_files] Found unknown entry type: {}", name_str);
                    }
                } else {
                    println!("[get_files] Entry without filename found");
                }
            }

            println!(
                "[get_files] Returning file list response with {} items",
                items.len()
            );
            Json(List {
                list: ApiCalls::FileList(items),
            })
            .into_response()
        }
        Err(e) => {
            eprintln!("[get_files] Failed to read directory: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to read directory",
            )
                .into_response()
        }
    }
}

// async fn get_files(
//     State(arc_state): State<Arc<RwLock<AppState>>>,
//     Query(params): Query<FileParams>
// ){

// }
// #[derive(Deserialize)]
// pub struct FileParams {
//     file: String,
// }

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

        #[cfg(all(not(feature = "full-stack"), not(feature = "database")))]
        async fn create_db_for_tests() -> Result<Database, String> {
            Ok(Database::new(None))
        }

        #[cfg(any(feature = "full-stack", feature = "database"))]
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
            let user = CreateElementData {
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
                .remove_user_in_db(RemoveElementData {
                    element: "kk".to_string(),
                    jwt: "".to_string(),
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
            let user = CreateElementData {
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
            let user = CreateElementData {
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
            let user = CreateElementData {
                element: Element::User {
                    user: "b".to_owned(),
                    password: "ddd".to_owned(),
                    user_perms: vec![],
                },
                require_auth: true,
                jwt: "".to_owned(),
            };
            let create_user_result = database.create_user_in_db(user).await;
            println!("{:#?}", create_user_result);
            let edit_user = CreateElementData {
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
            let user = CreateElementData {
                element: Element::User {
                    user: "A".to_owned(),
                    password: "a".to_owned(),
                    user_perms: vec![],
                },
                require_auth: true,
                jwt: "".to_owned(),
            };
            let create_user_result = database.create_user_in_db(user).await;
            let edit_user = CreateElementData {
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
            let user = CreateElementData {
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
            let userA = CreateElementData {
                element: Element::User {
                    user: "A".to_owned(),
                    password: "test".to_owned(),
                    user_perms: vec![],
                },
                require_auth: true,
                jwt: "".to_owned(),
            };
            let userB = CreateElementData {
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
