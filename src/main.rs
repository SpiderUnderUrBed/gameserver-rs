// ALOT of imports, needed given the size of this project in what it covers
// first imports are std ones
use std::collections::HashMap;
use std::convert::Infallible;
use std::default;
use std::fmt;
use std::fmt::Debug;
use std::{net::SocketAddr, path::Path, sync::Arc};

// Axum is the routing framework, and the backbone to this project helping intergrate the backend with the frontend
// and the general api, redirections, it will take form data and queries and make it easily accessible
// I also use axum_login to take off alot of effort that would be required for authentication
use crate::database::{DatabaseError, Element};
use crate::filesystem::TcpFileStream;
use crate::filesystem::{FsType, send_multipart_over_broadcast};
use crate::http::HeaderMap;
use crate::kubernetes::verify_is_k8s_gameserver;
use crate::middleware::from_fn;
use axum::Form;
use axum::extract::Multipart;
use axum::extract::Query;
use axum::extract::ws::Message as WsMessage;
use axum::http::Uri;
use axum::middleware::{self, Next};
use axum::response::Redirect;
use axum::{
    Router,
    body::Body,
    extract::{
        Request, State,
        ws::{Message, WebSocket, WebSocketUpgrade},
    },
    http::{self, Method, Response, StatusCode},
    response::{
        Html, IntoResponse, Json,
        sse::{Event, Sse},
    },
    routing::{get, post},
};
use axum_login::AuthUser;
use axum_login::tower_sessions::{MemoryStore, SessionManagerLayer};
use axum_login::{AuthManagerLayerBuilder, AuthnBackend};
use futures_util::FutureExt;
use serde::de::{self, DeserializeOwned};
use tokio::fs::File;
use tokio::sync::{RwLock, mpsc};

use rcon::Connection;

// mod databasespec;
// use databasespec::UserDatabase;
use crate::database::Node;
use crate::database::databasespec::Intergration;
use crate::database::databasespec::NodeType;
use crate::database::databasespec::NodesDatabase;
use crate::database::databasespec::UserDatabase;
use crate::database::databasespec::{Button, NodeStatus};
use crate::database::databasespec::{
    ButtonsDatabase, IntergrationsDatabase, K8sType, Server, ServerDatabase, Settings,
    SettingsDatabase,
};

use crate::http::header;
// miscellancious imports, future traits are used because alot of the code is asyncronus and cant fully be contained in tokio
// mime_guess as when I am serving the files, I need to serve it with the correct mime type
// serde_json because I exchange alot of json data between the backend and frontend and to the gameserver
// tokio because when working with alot of networking stuff and things that will take a indeterminent amount of time, async/await is the way to go (for better efficency too)
// chrono for time, tower for cors (TODO:: use less permissive CORS due to potential security risks)
// jsonwebtokens is standard when working with authentication, and bcrypt so I can use password hashs, I explain the authentication methods later
use async_trait::async_trait;
use chrono::{Duration as OtherDuration, Utc};
use futures_util::{sink::SinkExt, stream::StreamExt};
use jsonwebtoken::{DecodingKey, EncodingKey, Header, TokenData, Validation, decode, encode};
use mime_guess::from_path;
use serde;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use serial_test::serial;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::time::{Instant, interval, sleep};
use tokio::{
    fs as tokio_fs,
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    sync::{Mutex, broadcast},
    time::{Duration, timeout},
};
use tower_http::cors::{Any as CorsAny, CorsLayer};

use futures_util::task::Poll;
use futures_util::{Stream, TryFutureExt, stream};

use std::error::Error;
use std::sync::atomic::{AtomicUsize, Ordering};

use sysinfo::System;

static CONNECTION_COUNTER: AtomicUsize = AtomicUsize::new(0);
static FORWARD_ALL_MESSAGES: bool = true;

// For now I only restrict the json backend for running this without kubernetes
// the json backend is only for testing in most cases, simple deployments would use full-stack feature flag
// and you can use postgres manually with the database feature flag
#[cfg(any(feature = "full-stack", feature = "docker", feature = "database"))]
mod database {
    include!("pgdatabase.rs");
}

#[cfg(all(
    not(feature = "full-stack"),
    not(feature = "docker"),
    not(feature = "database")
))]
mod database {
    include!("jsondatabase.rs");
}

// JsonDatabase is only something that would be unique to Json and not any other database managed by sqlx
#[cfg(all(
    not(feature = "full-stack"),
    not(feature = "docker"),
    not(feature = "database")
))]
use database::JsonBackend;

// Both database files and any more should have these structs
use crate::database::databasespec::RetrieveElement;
use database::ModifyElementData;
use database::User;

// mod intergrations;
// use intergrations::{IntergrationCommands, run_intergration_commands};

mod extra;
use extra::value_from_line;

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
        _: String,
    ) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
        Ok(false)
    }
    pub async fn get_avalible_gameserver(
        _: &crate::Client,
    ) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        Err("This should not be running in a non-k8s environemnt".into())
    }
    pub async fn verify_is_k8s_pod(
        client: &crate::Client,
        ip: String,
    ) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
        Ok(false)
    }
    pub async fn verify_is_k8s_node(
        client: &crate::Client,
        ip: String,
    ) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
        Ok(false)
    }
}

// This part would potentially be removed later
// I like these defaults for testing, and for the moment I doubt anyone would object
// but at some point this will be removed in favor of testing with ENV varibles
#[cfg(not(feature = "full-stack"))]
static StaticTcpUrl: &str = "127.0.0.1:8082";

#[cfg(not(feature = "full-stack"))]
static StaticLocalUrl: &str = "127.0.0.1:8081";

#[cfg(not(feature = "full-stack"))]
static K8S_WORKS: bool = false;

#[cfg(not(feature = "docker"))]
static DOCKER_WORKS: bool = false;

#[cfg(feature = "full-stack")]
static StaticTcpUrl: &str = "gameserver-service:8080";

#[cfg(feature = "full-stack")]
static StaticLocalUrl: &str = "127.0.0.1:8080";

static WEBSOCKET_DEBUGGING: bool = false;
// K8S_WORKS needs to be true in the case where the full stack is running and not if that is not the case
// to avoid calling the dummy functions
#[cfg(feature = "full-stack")]
static K8S_WORKS: bool = true;

#[cfg(feature = "docker")]
static DOCKER_WORKS: bool = true;

// dummy client and function
#[cfg(not(feature = "full-stack"))]
#[derive(Clone, Debug)]
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
#[cfg(all(
    not(feature = "full-stack"),
    not(feature = "docker"),
    not(feature = "database")
))]
async fn first_connection() -> Result<JsonBackend, String> {
    Ok(JsonBackend::new(None))
}

// varibles which determines stuff about the tcp connection to the gameserver for data exchange
const CONNECTION_RETRY_DELAY: Duration = Duration::from_secs(2);
const CONNECTION_TIMEOUT: Duration = Duration::from_secs(3);
const CHANNEL_BUFFER_SIZE: usize = 32;

const ALLOW_NONJSON_DATA: bool = false;

// MessagePayload is how most data are exchanged between the gameserver, and the backend (sometimes the frontend)
// TODO: Consider merging CommandPayload, and IncomingMessage, and their corrosponding metadata varients
// (if i dont replace them with their metadata varients first), as originally I thought it would be good
// to know what data is passing through where but i think it might be creating more confusion
#[derive(Debug, Serialize, Deserialize)]
struct MessagePayload {
    r#type: String,
    message: String,
    authcode: String,
}
// MessagePayloadWithMetadata should replace MessagePayload at some point
// As the name implies, its the same aside from the metadata feild
// TODO: Replace MessagePayload with this
#[derive(Debug, Serialize, Deserialize)]
struct MessagePayloadWithMetadata {
    r#type: String,
    message: String,
    metadata: MetadataTypes,
    authcode: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct UnauthenticatedMessagePayload {
    r#type: String,
    message: String,
}

// #[derive(PartialEq)]
#[derive(Default)]
struct NodeAndTCP {
    name: String,
    ip: String,
    status: Status,
    nodetype: NodeType,
    k8s_type: K8sType,
    gameserver: Value,
    tcp_tx: Option<tokio::sync::broadcast::Sender<Vec<u8>>>,
    tcp_rx: Option<tokio::sync::broadcast::Receiver<Vec<u8>>>,
}
impl Clone for NodeAndTCP {
    fn clone(&self) -> NodeAndTCP {
        NodeAndTCP {
            name: self.name.clone(),
            ip: self.ip.clone(),
            nodetype: self.nodetype.clone(),
            status: self.status.clone(),
            gameserver: self.gameserver.clone(),
            tcp_tx: self.tcp_tx.clone(),
            tcp_rx: self.tcp_tx.as_ref().map(|tx| tx.subscribe()),
            k8s_type: self.k8s_type.clone(),
        }
    }
}

// more modern version of incoming message, i only keep incomingMessage for now as it will take a bit of effort to change it all to support the new types
// TODO: replace IncomingMessage with IncomingMessageWithMetadata
#[derive(Debug, Deserialize, Serialize, Clone)]
struct IncomingMessageWithMetadata {
    message: String,
    #[serde(rename = "type")]
    message_type: String,
    metadata: MetadataTypes,
    authcode: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(tag = "kind", content = "data")]
enum MetadataTypes {
    None,
    Server {
        servername: String,
        provider: String,
        providertype: String,
        location: String,
    },
    String(String),
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(tag = "kind", content = "data")]
enum IntegrationCommands {
    // MinecraftEnableRcon(serde_json::Value),
    // MinecraftDisableRcon(serde_json::Value),
    MinecraftEnableRcon(serde_json::Value),
    MinecraftDisableRcon(serde_json::Value),
}

#[derive(Clone)]
enum Clients {
    K8s(Client),
    Docker(String),
    None,
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

#[derive(Debug, Deserialize, Serialize)]
struct Statistics {
    used_memory: String,
    total_memory: String,
    core_data: Vec<String>,
    metadata: String,
}
// a list for things like nodes, capabilities, etc
#[derive(Debug, Serialize, Deserialize)]
struct List {
    list: ApiCalls,
}

// #[derive(Debug, serde::Deserialize)]
// struct KeywordPayload {
//     start_keyword: Option<String>,
//     stop_keyword: Option<String>,
// }

// May be redundant, but this is a struct for incoming messages
#[derive(Debug, Deserialize, Serialize, Clone)]
struct IncomingMessage {
    message: String,
    #[serde(rename = "type")]
    message_type: String,
    authcode: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
struct AuthTcpMessage {
    password: String,
}

// useful and not outdated because in this case the message is a Value, as in its not a predefined data type
#[derive(Debug, Deserialize, Serialize, Clone)]
struct IncomingMessageWithValue {
    message: Value,
    #[serde(rename = "type")]
    message_type: String,
    authcode: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
struct SrcAndDest {
    src: ApiCalls,
    dest: ApiCalls,
    metadata: String,
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
    NodeDataList(Vec<Node>),
    //NodeList(Vec<String>),
    IntergrationsDataList(Vec<Intergration>),
    UserData(LoginData),
    UserDataList(Vec<User>),
    ServerDataList(Vec<Server>),
    ButtonDataList(Vec<Button>),
    IncomingMessage(IncomingMessage),
    IncomingMessageWithMetadata(IncomingMessageWithMetadata),
    FileDataList(Vec<FsItem>),
    Node(Node),
    FileMoveOperation(String),
    FileCopyOperation(String),
    FileZipOperation(String),
    FileUnzipOperation(String),
    FileDownloadOperation(String),
    FileDownloadAllOperation(String),
    FileUploadAllOperation(String),
}
impl ApiCalls {
    fn as_inner_str(&self) -> Option<&str> {
        match self {
            ApiCalls::FileDownloadOperation(s) => Some(s),
            ApiCalls::FileZipOperation(s) => Some(s),
            ApiCalls::FileMoveOperation(s) => Some(s),
            ApiCalls::FileUnzipOperation(s) => Some(s),
            ApiCalls::FileCopyOperation(s) => Some(s),
            ApiCalls::FileUploadAllOperation(s) => Some(s),
            ApiCalls::FileDownloadAllOperation(s) => Some(s),
            _ => None, // handle other variants if needed
        }
    }
}

impl fmt::Display for ApiCalls {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            ApiCalls::FileDownloadOperation(_) => "FileDownloadOperation",
            ApiCalls::FileZipOperation(_) => "FileZipOperation",
            ApiCalls::FileMoveOperation(_) => "FileMoveOperation",
            ApiCalls::FileUnzipOperation(_) => "FileUnzipOperation",
            ApiCalls::FileCopyOperation(_) => "FileCopyOperation",
            ApiCalls::FileUploadAllOperation(_) => "FileUploadAllOperation",
            ApiCalls::FileDownloadAllOperation(_) => "FileDownloadAllOperation",
            _ => "not implemented",
        };
        write!(f, "{}", s)
    }
}

// #[derive(Debug, Deserialize, Serialize, Clone)]
// #[serde(tag = "kind", content = "data")]
// enum FileOperations {
//     Move(SrcAndDest)
// }

// struct ApiCallsWithAuth {
//     jwt: String,
//     require_auth: String,
//     api_call: ApiCalls
// }
use serde::Deserializer;
#[derive(Clone, Default, Serialize, Deserialize, Debug)]
enum Status {
    Unknown,
    Up,
    Healthy,
    #[default]
    Down,
    Unhealthy,
}

// impl<'de> Deserialize<'de> for Status {
//     fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
//     where
//         D: Deserializer<'de>,
//     {
//         let s = String::deserialize(deserializer)?.to_lowercase();
//         Ok(match s.as_str() {
//             "up" => Status::Up,
//             "healthy" => Status::Healthy,
//             "down" => Status::Down,
//             "unhealthy" => Status::Unhealthy,
//             _ => Status::Unknown,
//         })
//     }
// }

// AppState, this is a global struct which will be used to store data needed across the application like in routes and etc
// which includes the sender and reciver to the tcp connection for gameserver, the websocket sender (receiver only needs to be managed by its own handler)
// the base path like if all the routes are prefixed with something like /gameserver-rs which is the default for my testing deployment, and database as its needed frequently
// for user information and etc
// #[derive(Clone)]
struct AppState {
    tcp_tx: tokio::sync::broadcast::Sender<Vec<u8>>,
    tcp_rx: tokio::sync::broadcast::Receiver<Vec<u8>>,
    tcp_conn_status: Status,
    internal_rx: Option<broadcast::Receiver<Vec<u8>>>,
    internal_tx: Option<broadcast::Sender<Vec<u8>>>,
    additonal_node_tcp: Vec<NodeAndTCP>,
    //additonal_node_tcp: Vec<(tokio::sync::broadcast::Sender<Vec<u8>>, tokio::sync::broadcast::Receiver<Vec<u8>>)>,
    current_node: NodeAndTCP,
    //gameserver: Value,
    //status: Status,
    ws_tx: broadcast::Sender<String>,
    base_path: String,
    //client: Option<Client>,
    client: Clients,
    database: database::Database,
    cached_status_type: String,
    rcon_connection: Option<Arc<Mutex<Connection<TcpStream>>>>,
    current_server: Option<String>, 
    //sysinfo: System
}
impl Clone for AppState {
    fn clone(&self) -> Self {
        AppState {
            tcp_tx: self.tcp_tx.clone(),
            tcp_rx: self.tcp_rx.resubscribe(),
            internal_rx: self.internal_rx.as_ref().map(|r| r.resubscribe()),
            internal_tx: self.internal_tx.clone(),
            //gameserver: self.gameserver.clone(),
            //status: self.status.clone(),
            ws_tx: self.ws_tx.clone(),
            base_path: self.base_path.clone(),
            client: self.client.clone(),
            current_node: self.current_node.clone(),
            database: self.database.clone(),
            tcp_conn_status: Status::Unknown,
            //sysinfo: System::new_all(),
            additonal_node_tcp: self
                .additonal_node_tcp
                .iter()
                .map(|node| NodeAndTCP {
                    name: node.name.clone(),
                    ip: node.ip.clone(),
                    tcp_tx: node.tcp_tx.clone(),
                    tcp_rx: {
                        if let Some(tcp_rx) = &node.tcp_rx {
                            Some(tcp_rx.resubscribe())
                        } else {
                            None
                        }
                    },
                    nodetype: node.nodetype.clone(),
                    status: node.status.clone(),
                    gameserver: node.gameserver.clone(),
                    k8s_type: node.k8s_type.clone(),
                })
                .collect(),
            cached_status_type: String::new(),
            rcon_connection: self.rcon_connection.clone(),
            current_server: self.current_server.clone(),
        }
    }
}

// for the initial connection attempt, which will determine if possibly I would need to create the container and deployment upon failure
// i will use rusts 'timeout' for x interval determined with CONNECTION_TIMEOUT
async fn attempt_connection(
    tcp_url: String,
) -> Result<TcpStream, Box<dyn std::error::Error + Send + Sync>> {
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
    mut internal_stream: Option<broadcast::Receiver<Vec<u8>>>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let ip = stream.peer_addr()?.to_string();
    let (reader, mut writer) = stream.split();
    let mut buf_reader = BufReader::new(reader);
    let mut buf = vec![0u8; 4096];

    let initial_node_password: String =
        get_env_var_or_arg("INITIAL_NODE_PASSWORD", Some(String::default())).unwrap();
    let auth_msg = serde_json::to_string(&AuthTcpMessage {
        password: initial_node_password,
    })? + "\n";
    writer.write_all(auth_msg.as_bytes()).await?;

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

    async fn process_stream_data(
        raw_data: &[u8],
        arc_state: &Arc<RwLock<AppState>>,
        ws_tx: &broadcast::Sender<String>,
        ip: &str,
        server_start_keyword: &mut String,
        server_stop_keyword: &mut String,
    ) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
        {
            let state_guard = arc_state.read().await;
            let _ = state_guard.tcp_tx.send(raw_data.to_vec());
        }

        if let Ok(text) = std::str::from_utf8(raw_data) {
            let line_content = text.trim();
            if line_content.is_empty() {
                return Ok(false);
            }

            let mut final_data: Vec<Value> = vec![];

            let list_parsed: Vec<Result<List, serde_json::Error>> =
                value_from_line::<List, _>(line_content, |line| line.contains("\"list\"")).await;

            let mut list_values: Vec<Value> = vec![];
            for item in list_parsed {
                if let Ok(list_item) = item {
                    let serialized = serde_json::to_string(&list_item)?;
                    if let Ok(seralized_value) = serde_json::to_value(ConsoleData {
                        data: serialized,
                        r#type: "list_item".to_string(),
                        authcode: "0".to_string(),
                    }) {
                        list_values.push(seralized_value)
                    }
                }
            }
            let mut list_lines: Vec<String> = list_values
                .iter()
                .map(|v| {
                    serde_json::to_string(v.get("data").clone().unwrap_or(&Value::Null))
                        .unwrap_or(String::new())
                })
                .collect();

            let console_parsed: Vec<Result<ConsoleData, serde_json::Error>> =
                value_from_line::<ConsoleData, _>(line_content, |line| !line.contains("\"list\""))
                    .await;

            let mut console_values: Vec<Value> = vec![];
            for item in console_parsed {
                if let Ok(data) = item {
                    if !list_lines.contains(&data.data) {
                        if let Ok(seralized_value) = serde_json::to_value(data) {
                            console_values.push(seralized_value);
                        }
                    }
                }
            }
            let message_parsed: Vec<Result<MessagePayload, serde_json::Error>> =
                value_from_line::<MessagePayload, _>(line_content, |line| {
                    !line.contains("\"list\"")
                })
                .await;

            //println!("value: \n {:#?}, line: \n {}", message_parsed, line_content);
            let mut message_values: Vec<Value> = vec![];
            for item in message_parsed {
                if let Ok(data) = item {
                    if let Ok(seralized_value) = serde_json::to_value(data) {
                        message_values.push(seralized_value)
                    }
                }
            }
            let src_and_dest_parsed: Vec<Result<SrcAndDest, serde_json::Error>> =
                value_from_line::<SrcAndDest, _>(line_content, |line| line.contains("\"src\""))
                    .await;

            let mut src_and_dest_values: Vec<Value> = vec![];
            for item in src_and_dest_parsed {
                if let Ok(data) = item {
                    if let Ok(serialized_value) = serde_json::to_value(data) {
                        src_and_dest_values.push(serialized_value);
                    }
                }
            }

            let integration_parsed: Vec<Result<IntegrationCommands, serde_json::Error>> =
                value_from_line::<IntegrationCommands, _>(line_content, |line| {
                    line.contains("\"kind\"")
                })
                .await;

            let mut integration_values: Vec<Value> = vec![];
            for item in integration_parsed {
                if let Ok(data) = item {
                    if let Ok(serialized_value) = serde_json::to_value(data) {
                        integration_values.push(serialized_value);
                    }
                }
            }

            final_data.extend(src_and_dest_values);
            final_data.extend(integration_values);
            final_data.extend(list_values);
            final_data.extend(console_values);
            final_data.extend(message_values);
            //println!("{:#?}", final_data);

            for value in final_data.iter() {
                //println!("{:#?}", value);
                if let Ok(payload) = serde_json::from_value::<MessagePayload>(value.clone()) {
                    //println!("{:#?}", payload);
                    if payload.message == "end_conn" {
                        println!("Ending current connection");
                        let mut state_guard = arc_state.write().await;
                        state_guard.tcp_conn_status = Status::Down;
                        return Ok(true);
                    } else if payload.r#type == "server_state" {
                        //println!("Got server state");
                        let mut state_guard = arc_state.write().await;
                        let sent_status = payload.message.parse().unwrap_or(false);
                        state_guard.current_node.status = match sent_status {
                            true => Status::Up,
                            false => Status::Down,
                        }
                    }
                }

                if let Ok(data_clone) = serde_json::from_value::<ConsoleData>(value.clone()) {
                    // println!("{:#?}", data_clone);
                    if let Ok(inner_value) =
                        serde_json::from_str::<serde_json::Value>(&data_clone.data)
                    {
                        // println!("{:#?}", inner_value);
                        if let (Some(start_kw), Some(stop_kw)) = (
                            inner_value.get("start_keyword").and_then(|v| v.as_str()),
                            inner_value.get("stop_keyword").and_then(|v| v.as_str()),
                        ) {
                            //println!("Changing keyword {} {}", start_kw.to_string().clone(), stop_kw.to_string().clone());
                            *server_start_keyword = start_kw.to_string();
                            *server_stop_keyword = stop_kw.to_string();
                        }
                    }

                    if data_clone.data.contains("\"type\":\"command\"") {
                        if let Ok(inner_msg) =
                            serde_json::from_str::<MessagePayload>(&data_clone.data)
                        {
                            if inner_msg.r#type == "command" {
                                let (client_option, database) = {
                                    let state_guard = arc_state.read().await;
                                    (state_guard.client.clone(), state_guard.database.clone())
                                };

                                if let Ok(nodes) = database.fetch_all_nodes().await {
                                    let node_status = if let Clients::K8s(client) = client_option {
                                        let client_clone = client.clone();
                                        let ip_clone = ip.to_string();

                                        match tokio::time::timeout(
                                            std::time::Duration::from_millis(100),
                                            verify_is_k8s_gameserver(client_clone, ip_clone),
                                        )
                                        .await
                                        {
                                            Ok(Ok(true)) => NodeStatus::ImmutablyEnabled,
                                            _ => NodeStatus::Enabled,
                                        }
                                    } else {
                                        NodeStatus::Enabled
                                    };

                                    let node = Node {
                                        ip: ip.to_string(),
                                        nodename: inner_msg.message,
                                        nodetype: {
                                            let state_guard = arc_state.read().await;
                                            if let Clients::K8s(client) = &state_guard.client {
                                                if kubernetes::verify_is_k8s_gameserver(
                                                    client.clone(),
                                                    ip.to_string(),
                                                )
                                                .await?
                                                {
                                                    NodeType::Inbuilt
                                                } else {
                                                    NodeType::Custom
                                                }
                                            } else {
                                                NodeType::Custom
                                            }
                                        },
                                        nodestatus: node_status,
                                        k8s_type: {
                                            let state_guard = arc_state.read().await;
                                            if let Clients::K8s(client) = &state_guard.client {
                                                if kubernetes::verify_is_k8s_gameserver(
                                                    client.clone(),
                                                    ip.to_string(),
                                                )
                                                .await?
                                                {
                                                    if kubernetes::verify_is_k8s_pod(
                                                        client,
                                                        ip.to_string(),
                                                    )
                                                    .await?
                                                    {
                                                        K8sType::Pod
                                                    } else if kubernetes::verify_is_k8s_node(
                                                        client,
                                                        ip.to_string(),
                                                    )
                                                    .await?
                                                    {
                                                        K8sType::Node
                                                    } else {
                                                        K8sType::Unknown
                                                    }
                                                } else {
                                                    K8sType::None
                                                }
                                            } else {
                                                K8sType::Unknown
                                            }
                                        },
                                    };

                                    if !nodes
                                        .iter()
                                        .any(|n| n.ip == node.ip && n.nodename == node.nodename)
                                    {
                                        let _ = database
                                            .create_nodes_in_db(ModifyElementData {
                                                element: Element::Node(node),
                                                jwt: "".to_string(),
                                                require_auth: false,
                                            })
                                            .await;
                                    }
                                }
                            }
                        }
                    }

                    if data_clone.data.contains("\"type\":\"stdout\"") {
                        if let Ok(output_msg) =
                            serde_json::from_str::<serde_json::Value>(&data_clone.data)
                        {
                            if let Some(server_output) =
                                output_msg.get("data").and_then(|v| v.as_str())
                            {
                                if !server_start_keyword.is_empty()
                                    && server_output.contains(&*server_start_keyword)
                                {
                                    let _ =
                                        ws_tx.send("Server is ready for connections!".to_string());
                                    {
                                        let mut state_guard = arc_state.write().await;
                                        state_guard.current_node.status = Status::Up;
                                    }
                                } else if !server_stop_keyword.is_empty()
                                    && server_output.contains(&*server_stop_keyword)
                                {
                                    {
                                        let mut state_guard = arc_state.write().await;
                                        state_guard.current_node.status = Status::Down;
                                    }
                                }
                                let _ = ws_tx.send(server_output.to_string());
                                continue;
                            }
                        }
                    }

                    if !data_clone.data.contains("\"type\":\"stdout\"")
                        && !data_clone.data.contains("\"type\":\"command\"")
                    {
                        let _ = ws_tx.send(data_clone.data.clone());
                    }
                }

                if FORWARD_ALL_MESSAGES == true {
                    let all_remaining_messages_result = match serde_json::to_string(&value) {
                        Ok(string) => {
                            // internal_tx
                            if let Err(err) = ws_tx.send(string) {
                                eprintln!(
                                    "Failed to send request over broadcast: {} (not fatal)",
                                    err
                                );
                            }
                        }
                        Err(err) => eprintln!("Failed to serialize request: {}", err),
                    };
                }
                //let _ = ws_tx.send(final_value);
            }
        }
        Ok(false)
    }

    loop {
        if let Some(ref mut internal_rx) = internal_stream {
            tokio::select! {
                read_result = buf_reader.read(&mut buf) => {
                    match read_result {
                        Ok(0) => break,
                        Ok(n) => {
                            let raw_data = &buf[..n];
                            if process_stream_data(raw_data, &arc_state, &ws_tx, &ip, &mut server_start_keyword, &mut server_stop_keyword).await? {
                                break;
                            }
                        }
                        Err(e) => return Err(e.into()),
                    }
                }

                internal_result = internal_rx.recv() => {
                    if let Ok(raw_data) = internal_result {
                        if process_stream_data(&raw_data, &arc_state, &ws_tx, &ip, &mut server_start_keyword, &mut server_stop_keyword).await? {
                            break;
                        }
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
        } else {
            tokio::select! {
                read_result = buf_reader.read(&mut buf) => {
                    match read_result {
                        Ok(0) => break,
                        Ok(n) => {
                            let raw_data = &buf[..n];
                            if process_stream_data(raw_data, &arc_state, &ws_tx, &ip, &mut server_start_keyword, &mut server_stop_keyword).await? {
                                break;
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
    }

    Ok(())
}

// use tokio::sync::broadcast::error::RecvError::Lagged;
// use tokio::sync::broadcast::error::RecvError::Closed;

// does the connection to the tcp server, wether initial or not, on success it will pass it off to the dedicated handler for the stream
pub async fn connect_to_server(
    arc_state: Arc<RwLock<AppState>>,
    tcp_url: String,
    mut rx: broadcast::Receiver<Vec<u8>>,
    ws_tx: broadcast::Sender<String>,
    internal_stream: Option<broadcast::Receiver<Vec<u8>>>,
    end_if_timeout: bool,
    block_with_stream: bool,
    //overall_timeout: Option<Duration>,
) -> Result<SocketAddr, Box<dyn Error + Send + Sync>> {
    let deadline: Instant = Instant::now() + CONNECTION_TIMEOUT;
    //overall_timeout.unwrap_or(CONNECTION_TIMEOUT);

    loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return Err("overall connect timeout".into());
        }

        let per_attempt = CONNECTION_TIMEOUT.min(remaining);

        match timeout(per_attempt, TcpStream::connect(&tcp_url)).await {
            Ok(Ok(mut stream)) => {
                let peer = stream.peer_addr()?;
                let st = Arc::clone(&arc_state);
                let tx = ws_tx.clone();

                let mut state_guard = arc_state.write().await;
                state_guard.tcp_conn_status = Status::Up;

                if !block_with_stream {
                    tokio::spawn(async move {
                        if let Err(e) =
                            handle_stream(st, &mut rx, &mut stream, tx, internal_stream).await
                        {
                            eprintln!("handle_stream error: {}", e);
                        }
                    });
                } else {
                    if let Err(e) =
                        handle_stream(st, &mut rx, &mut stream, tx, internal_stream).await
                    {
                        eprintln!("handle_stream error: {}", e);
                    }
                }
                return Ok(peer);
            }
            Ok(Err(e)) => {
                eprintln!("TCP connect error: {}", e);
                let mut state_guard = arc_state.write().await;
                if let Some(tx) = &state_guard.internal_tx {
                    tx.send("end_conn".into());
                }
                state_guard.tcp_conn_status = Status::Down;
                // if check_channel_health(&state_guard.tcp_tx, state_guard.tcp_rx.resubscribe()).await {
                //     // println!("up");
                //     state_guard.tcp_conn_status = Status::Up
                // } else {
                //     //println!("down");
                //     state_guard.tcp_conn_status = Status::Down
                // }
                println!("Done");
            }
            Err(_) => {
                let mut state_guard = arc_state.write().await;
                state_guard.tcp_conn_status = Status::Down;
                eprintln!("TCP connect timed out ({:?})", per_attempt);
                if end_if_timeout {
                    return Err("connection attempt timed out".into());
                }
            }
        }

        sleep(CONNECTION_RETRY_DELAY).await;
    }
}

// this is where it determines wether or not to try and create the container and deployment, as attempt_connection itself is used in various diffrent contexts (like it will constantly
// try to connect upon failing but it should not try to create the container and deployment every time it fails)
async fn try_initial_connection(
    conn_attempts: u64,
    conn_timeout: u64,
    create_handler: bool,
    state: &Arc<RwLock<AppState>>,
    tcp_url: String,
    ws_tx: &broadcast::Sender<String>,
    tcp_tx: tokio::sync::broadcast::Sender<Vec<u8>>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let mut final_error: Box<dyn std::error::Error + Send + Sync> =
        "This should not show up".into();
    for _ in 0..conn_attempts {
        // let mut has_succeeded = false;
        match attempt_connection(tcp_url.clone()).await {
            Ok(mut stream) => {
                //let copy_state = &Arc::clone(&state);
                println!("Initial connection succeeded!");
                // note, possibly I wont ever need to create a handler from the test of the intial connection
                // TODO: think about removing create_handler and just never create a handler
                if create_handler {
                    let (temp_tx, temp_rx) =
                        tokio::sync::broadcast::channel::<Vec<u8>>(CHANNEL_BUFFER_SIZE);
                    let mut temp_rx = temp_rx;
                    let stream_result = handle_stream(
                        state.clone(),
                        &mut temp_rx,
                        &mut stream,
                        ws_tx.clone(),
                        None,
                    )
                    .await;
                    if stream_result.is_ok() {
                        break;
                    } else {
                        final_error = stream_result.err().unwrap()
                    }
                    //.map_err(|| "Failed conn attempt")?
                } else {
                    break;
                }
            }
            Err(e) => {
                eprintln!("Initial connection failed: {}", e);
                // return Err(e)
            }
        }
        tokio::time::sleep(Duration::from_secs(2)).await;
    }
    Err(final_error)
}

// fn get_arg_or_env_var<T: std::str::FromStr>(env_var: &str, arg: Option<T>) -> Option<T> {
//     arg.or_else(|| env::var(env_var).ok().and_then(|s| s.parse().ok()))
// }

// Looks for a env varible, if its not found, try the specified default, if none is found it will use the default of whatever that type is
// fn get_env_var_or_arg<T: std::str::FromStr>(env_var: &str, default: Option<T>) -> Option<T> {
//     env::var(env_var).ok().and_then(|s| s.parse().ok()).or(default)
// }
fn get_env_var_or_arg<T>(env_var: &str, default: Option<T>) -> Option<T>
where
    T: std::str::FromStr + Clone,
{
    env::var(env_var)
        .ok()
        .and_then(|s| s.parse::<T>().ok())
        .or(default)
}

// fn get_env_var_or_required_arg<T: std::str::FromStr>(env_var: &str, arg: Option<T>, field_name: &str) -> T {
//     get_env_var_or_arg(env_var, arg).unwrap_or_else(|| {
//         panic!("{} must be provided either as argument or through {}", field_name, env_var)
//     })
// }

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

    let config_tcp_url = get_env_var_or_arg("TCPURL", Some(StaticTcpUrl.to_string())).unwrap();
    let config_local_url =
        get_env_var_or_arg("LOCALURL", Some(StaticLocalUrl.to_string())).unwrap();

    // Overrides for testing or specific cases where how it worksin a setup may be diffrent

    let enable_k8s_client: bool = get_env_var_or_arg("ENABLE_K8S_CLIENT", Some(true)).unwrap();
    let force_rebuild: bool = get_env_var_or_arg("FORCE_REBUILD", Some(false)).unwrap();
    let build_docker_image: bool = get_env_var_or_arg("BUILD_DOCKER_IMAGE", Some(true)).unwrap();
    let build_deployment: bool = get_env_var_or_arg("BUILD_DEPLOYMENT", Some(true)).unwrap();
    let dont_override_conn_with_k8s: bool =
        get_env_var_or_arg("DONT_OVERRIDE_CONN_WITH_K8S", Some(true)).unwrap();

    // consider if I should not have enable_initial_connection and instead if initial_connection_attempts dont
    // try to connect to the server
    let enable_initial_connection: bool =
        get_env_var_or_arg("ENABLE_INITIAL_CONNECTION", Some(true)).unwrap();
    let initial_connection_attempts: u64 =
        get_env_var_or_arg("INITIAL_CONNECTION_ATTEMPTS", Some(5)).unwrap();
    let initial_connection_timeout: u64 =
        get_env_var_or_arg("INITIAL_CONNECTION_TIMEOUT", Some(2)).unwrap();

    // creates a websocket broadcase and tcp channels
    let (ws_tx, _) = broadcast::channel::<String>(CHANNEL_BUFFER_SIZE);
    let (tcp_tx, tcp_rx) = broadcast::channel::<Vec<u8>>(CHANNEL_BUFFER_SIZE);

    // sets the client to be none by default unless this is ran the stanard way which will be ran with the appropriate feature-flag
    // which will set the k8s client
    // let mut client: Option<Client> = None;
    // if enable_k8s_client && K8S_WORKS {
    //     client = Some(Client::try_default().await?);
    // }
    let mut client: Clients = Clients::None;
    if enable_k8s_client && K8S_WORKS {
        client = Clients::K8s(Client::try_default().await?);
    }

    let mut tcp_url: String = config_tcp_url.to_string();
    if !dont_override_conn_with_k8s && let Clients::K8s(ref inner_client) = client {
        //let Clients::K8s(inner_client) = client;
        if let Ok(url_result) = &kubernetes::get_avalible_gameserver(&inner_client).await {
            tcp_url = url_result.clone();
        } else {
            println!(
                "Could not get a successful url for a existing gameserver, will try the fallback url"
            )
        }
    }

    let mut nodes: Vec<NodeAndTCP> = vec![];
    if let Ok(db_nodes) = database.fetch_all_nodes().await {
        nodes = db_nodes
            .into_iter()
            .map(|node| NodeAndTCP {
                name: node.nodename,
                nodetype: node.nodetype,
                ip: node.ip,
                ..Default::default()
            })
            .collect()
    }
    let backup_node = &NodeAndTCP {
        name: "placeholder".to_string(),
        ip: tcp_url.clone(),
        ..Default::default()
    };
    let current_node = nodes
        .iter()
        .find(|node| node.ip == tcp_url)
        .unwrap_or(backup_node);
    let (internal_tx, internal_rx) = broadcast::channel::<Vec<u8>>(100);
    let internal_stream = Some(internal_rx.resubscribe());

    let mut rcon_connection: Option<Arc<Mutex<Connection<TcpStream>>>> = None;
    if let Ok(retrived_db) = database.get_settings().await {
        if (retrived_db.enabled_rcon) {
            rcon_connection = match Connection::builder()
                .enable_minecraft_quirks(true)
                .connect(&retrived_db.rcon_url, &retrived_db.rcon_password)
                .await
                .map_err(|e| Box::new(e) as Box<dyn Error + Send + Sync>)
                //)))
                {
                    Ok(conn) => Some(Arc::new(Mutex::new(conn))),
                    Err(e) => {
                        eprintln!("Failed to connect to RCON: {}", e);
                        None
                        // Connection stays None
                    }
                //retrived_db
                }
        }
    }

    let mut current_server = None;
    if !database.get_settings().await?.current_server.is_empty(){
        current_server = Some(database.get_settings().await?.current_server)
    }

    // use everything so far to make the app state
    let mut state: AppState = AppState {
        //gameserver: json!({}),
        //status: Status::Down,
        tcp_tx: tcp_tx,
        // tcp_rx: Arc::new(Mutex::new(tcp_rx)),
        tcp_rx: tcp_rx,
        internal_rx: Some(internal_rx.resubscribe()),
        internal_tx: Some(internal_tx),
        ws_tx: ws_tx.clone(),
        base_path: base_path.clone(),
        current_node: NodeAndTCP::default(),
        database,
        client,
        additonal_node_tcp: nodes,
        tcp_conn_status: Status::Unknown,
        cached_status_type: String::new(),
        rcon_connection,
        current_server, 
        //sysinfo: System::new_all()
    };
    state.tcp_conn_status = {
        if check_channel_health(&state.tcp_tx, state.tcp_rx.resubscribe()).await {
            Status::Up
        } else {
            Status::Down
        }
    };

    let multifaceted_state = Arc::new(RwLock::new(state));
    load_settings(multifaceted_state.clone()).await;

    // if there is supposed to be a initial connection and if there is a client (as it wont be able to create the deployment without it, and it would be pointless to create a docker container
    // without the abbility to deploy it)
    if enable_initial_connection {
        // && multifaceted_state.write().await.client.is_some() {
        println!("Trying initial connection...");
        let initial_connection_result = try_initial_connection(
            initial_connection_attempts,
            initial_connection_timeout,
            false,
            &multifaceted_state.clone(),
            tcp_url.to_string(),
            &ws_tx.clone(),
            multifaceted_state.write().await.tcp_tx.clone(),
        )
        .await;
        if initial_connection_result.is_err() {
            println!("All initial connections failed");
        }
        if initial_connection_result.is_err() || force_rebuild {
            //if matches!(multifaceted_state.write().await.client, Clients::K8s(_)) {
            if let Clients::K8s(client) = &multifaceted_state.write().await.client {
                eprintln!(
                    "Initial connection failed or force rebuild enabled, will possibly enable auto-build (configurable)"
                );
                if build_docker_image {
                    docker::build_docker_image().await?;
                }
                if build_deployment {
                    kubernetes::create_k8s_deployment(&client).await?;
                }
            }
        }
    }

    // takes the tcp connection out of the arc mutex and gets a connection to the
    // websocket to send to connect_to_server to establish the date pipeline
    let server_connection_state = multifaceted_state.clone();
    let bridge_rx = multifaceted_state.read().await.tcp_rx.resubscribe();
    let bridge_tx = multifaceted_state.read().await.ws_tx.clone();

    let arc_state_clone = multifaceted_state.clone();

    tokio::spawn(async move {
        if let Err(e) = connect_to_server(
            server_connection_state,
            tcp_url.to_string(),
            bridge_rx,
            bridge_tx,
            internal_stream,
            false,
            false,
        )
        .await
        {
            eprintln!("Connection task failed: {}", e);
            let mut temporary_state = arc_state_clone.write().await;
            // if check_channel_health(&temporary_state.tcp_tx, temporary_state.tcp_rx.resubscribe()).await {
            //     // println!("up");
            //     temporary_state.tcp_conn_status = Status::Up
            // } else {
            //     //println!("down");
            //     temporary_state.tcp_conn_status = Status::Down
            // }
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
        // .route("/api/fileupload", post(stream_file_download))
        .route("/api/download/{*wildcard}", get(stream_file_download))
        //.route("/api/uploadcontent", post(upload))
        .route("/api/statistics", get(statistics))
        .route("/api/getsettings", get(get_settings))
        .route("/api/awaitserverstatus", get(ongoing_server_status))
        .route("/api/intergrations", get(get_integrations))
        .route("/api/createintergrations", post(create_intergration))
        .route("/api/modifyintergrations", post(modify_intergration))
        .route("/api/deleteintergrations", post(delete_intergration))
        // Actual intergrations
        .route("/api/rconcommand", post(rcon_command))
        // End of actual intergrations
        .route("/api/refreshstatus", post(refresh_status))
        .route("/api/setsettings", post(set_settings))
        .route("/api/changenode", post(change_node))
        .route("/api/fetchnode", post(fetch_node))
        .route("/api/migrate", post(migrate))
        .route("/api/getstatus", post(get_status))
        .route("/api/getfiles", post(get_files))
        .route("/api/getfilescontent", post(get_files_content))
        .route("/api/buttonreset", post(button_reset))
        .route("/api/editbuttons", post(edit_buttons))
        .route("/api/addnode", post(add_node))
        .route("/api/addserver", post(add_server))
        .route("/api/edituser", post(edit_user))
        .route("/api/getuser", post(get_user))
        .route("/api/setserver", post(set_server))
        .route("/api/getserver", post(get_server))
        .route("/api/send", post(receive_message))
        .route("/api/general", post(process_general))
        .route(
            "/api/generalwithmetadata",
            post(process_general_with_metadata),
        )
        // the above route is supposed to be the newer version to /api/general, adding a additonal metadata feild, changing all incomingmessages to incomingmessageswithmetadata
        // will take awhile, so temporarially ill have this route
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
    let addr: SocketAddr = config_local_url.parse().unwrap();
    println!("Listening on http://{}{}", addr, base_path);

    let listener = TcpListener::bind(addr).await?;
    axum::serve(listener, app.into_make_service()).await?;

    Ok(())
}
pub async fn rcon_command(
    State(arc_state): State<Arc<RwLock<AppState>>>,
    Json(request): Json<IncomingMessage>,
) -> impl IntoResponse {
    if let Err(e) = ensure_rcon(Arc::clone(&arc_state)).await {
        eprintln!("Failed to ensure RCON: {}", e);
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    }

    let state = arc_state.read().await;

    if let Some(arc_conn) = &state.rcon_connection {
        let mut conn = arc_conn.lock().await;

        match conn.cmd(&request.message).await {
            Ok(response) => Json(UnauthenticatedMessagePayload {
                r#type: "rcon_response".to_string(),
                message: response,
            })
            .into_response(),
            Err(e) => {
                eprintln!("RCON command error: {}", e);
                StatusCode::INTERNAL_SERVER_ERROR.into_response()
            }
        }
    } else {
        StatusCode::SERVICE_UNAVAILABLE.into_response()
    }
}

pub async fn ensure_rcon(arc_state: Arc<RwLock<AppState>>) -> Result<(), String> {
    let mut state = arc_state.write().await;

    if state.rcon_connection.is_none() {
        if let Ok(retrived_db) = state.database.get_settings().await {
            if retrived_db.enabled_rcon {
                match Connection::builder()
                    .enable_minecraft_quirks(true)
                    .connect(&retrived_db.rcon_url, &retrived_db.rcon_password)
                    .await
                {
                    Ok(conn) => {
                        state.rcon_connection = Some(Arc::new(Mutex::new(conn)));
                        return Ok(());
                    }
                    Err(e) => {
                        eprintln!("Failed to connect to RCON: {}", e);
                        return Err(format!("RCON connection failed: {}", e));
                    }
                }
            }
        }
        return Err("RCON not enabled or settings not available".to_string());
    }

    Ok(())
}
// async fn uploadcontent(
//     State(arc_state): State<Arc<RwLock<AppState>>>,
//     multipart: Multipart,
// ) -> StatusCode {

// }
async fn file_operations(
    State(arc_state): State<Arc<RwLock<AppState>>>,
    Json(request): Json<SrcAndDest>,
) -> StatusCode {
    let state = arc_state.write().await;

    match serde_json::to_vec(&request) {
        Ok(bytes) => {
            if let Err(err) = state.tcp_tx.send(bytes) {
                eprintln!("Failed to send request over broadcast: {}", err);
            }
        }
        Err(err) => eprintln!("Failed to serialize request: {}", err),
    }

    StatusCode::CREATED
}
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
async fn refresh_status(
    State(arc_state): State<Arc<RwLock<AppState>>>,
    // Json(_): Json<IncomingMessage>,
) {
    let mut state = arc_state.write().await;
    state.tcp_conn_status = {
        if check_channel_health(&state.tcp_tx, state.tcp_rx.resubscribe()).await {
            Status::Up
        } else {
            Status::Down
        }
    };
}

// TODO: maybe split this function and route into several routes with statuses for diffrent states/nodes/settings?
async fn get_status(
    State(arc_state): State<Arc<RwLock<AppState>>>,
    Json(request): Json<IncomingMessage>,
) -> impl IntoResponse {
    let state = arc_state.write().await;

    let mut returning_req = IncomingMessage {
        message: String::new(),
        message_type: "status".to_string(),
        authcode: "0".to_string(),
    };

    if request.message_type == "buttons" {
        match state.database.toggle_button_state().await {
            Ok(status) => {
                returning_req.message = status.to_string();
            }
            Err(_) => {
                returning_req.message = "error".to_string();
            }
        }
        Json(returning_req)
    } else if request.message_type == "node" {
        Json(returning_req)
    } else {
        Json(returning_req)
    }
}
async fn get_settings(State(state): State<Arc<RwLock<AppState>>>) -> impl IntoResponse {
    match state.read().await.database.get_settings().await {
        Ok(settings) => Json(settings).into_response(),
        Err(_err) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
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
        list: ApiCalls::ButtonDataList(button_list),
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
    r#type: String,
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

// #[axum::debug_handler]
async fn load_settings(
    arc_state: Arc<RwLock<AppState>>,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    let mut state = arc_state.write().await;
    let settings = match state.database.get_settings().await {
        Ok(s) => s,
        Err(_) => {
            return Err(Box::<dyn Error + Send + Sync>::from(
                "Failed to load settings from database",
            ));
        }
    };
    // println!("Loading settings");
    // println!("{:#?}", state.cached_status_type);
    state.cached_status_type = settings.status_type;

    Ok(())
}

async fn set_settings(
    State(arc_state): State<Arc<RwLock<AppState>>>,
    Json(request): Json<IncomingMessageWithValue>,
) -> impl IntoResponse {
    let inner_value = request.message;
    //IncomingMessageWithValue
    let mut state = arc_state.write().await;
    let settings = match state.database.get_settings().await {
        Ok(s) => s,
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    };

    let mut settings_value = match serde_json::to_value(settings) {
        Ok(v) => v,
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    };

    if let (Value::Object(current), Value::Object(new)) = (&mut settings_value, inner_value) {
        for (k, v) in new {
            current.insert(k, v);
        }
    } else {
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    }

    let updated_settings: Settings = match serde_json::from_value(settings_value) {
        Ok(s) => s,
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    };
    let mut has_created = false;
    match state.database.set_settings(updated_settings).await {
        Ok(_) => has_created = true,
        _ => {}
    };
    drop(state);
    //println!("Starting it");
    let _ = load_settings(arc_state).await;
    //println!("past it");
    if has_created {
        StatusCode::CREATED.into_response()
    } else {
        StatusCode::INTERNAL_SERVER_ERROR.into_response()
    }
}

async fn statistics(
    State(_): State<Arc<RwLock<AppState>>>,
) -> Sse<impl Stream<Item = Result<Event, Box<dyn Error + Send + Sync>>>> {
    let interval = interval(Duration::from_secs(3));
    let system = System::new_all();
    //let state_clone = arc_state.clone();

    let updates = stream::unfold(
        (interval, system),
        move |(mut interval, mut system)| async move {
            interval.tick().await;
            system.refresh_all();
            // let state = arc_state.write().await;
            // let mut system = state.sysinfo;
            // system.refresh_all();

            let core_data: Vec<String> = system
                .cpus()
                .into_iter()
                .map(|core| core.cpu_usage().to_string())
                .collect();
            let statistics = Statistics {
                total_memory: system.total_memory().to_string(),
                used_memory: system.used_memory().to_string(),
                core_data,
                metadata: "".to_string(),
            };
            let event = match serde_json::to_string(&statistics) {
                Ok(json) => Ok(Event::default().data(json)),
                Err(_) => Err("Error".into()),
            };
            Some((event, (interval, system)))
            // drop(state);
            //Some((Ok(Event::default().data(serde_json::to_string(statistics)), (interval, system))))
        },
    );
    Sse::new(updates).keep_alive(axum::response::sse::KeepAlive::default())
}

async fn check_channel_health(
    tx: &broadcast::Sender<Vec<u8>>,
    mut rx: broadcast::Receiver<Vec<u8>>,
) -> bool {
    match tx.send("ping".into()) {
        Ok(_) => true,
        Err(_) => return false,
    };

    match rx.recv().await {
        Ok(_msg) => true,
        Err(broadcast::error::RecvError::Closed) => false,
        Err(broadcast::error::RecvError::Lagged(_)) => true,
    }
}

async fn ongoing_server_status(
    State(arc_state): State<Arc<RwLock<AppState>>>,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    let interval = interval(Duration::from_secs(3));
    let state_clone = arc_state.clone();

    let updates = stream::unfold(
        (interval, state_clone),
        move |(mut interval, arc_state)| async move {
            interval.tick().await;
            //println!("start");
            let status = {
                let state = arc_state.read().await;

                if state.cached_status_type.is_empty()
                    || state.cached_status_type == "server-keyword"
                {
                    state.current_node.status.clone()
                } else if state.cached_status_type == "server-process" {
                    Status::Unknown
                } else if state.cached_status_type == "node" {
                    state.tcp_conn_status.clone()
                } else {
                    Status::Unknown
                }
            };

            //println!("{:#?}", status);
            // println!("past");
            let status_str = match status {
                Status::Up => "up",
                Status::Healthy => "healthy",
                Status::Down => "down",
                Status::Unhealthy => "unhealthy",
                Status::Unknown => "unknown",
                _ => &String::new(),
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

async fn add_server(
    State(arc_state): State<Arc<RwLock<AppState>>>,
    Json(request): Json<ModifyElementData>,
) -> impl IntoResponse {
    let state = arc_state.write().await;
    let result = state
        .database
        .create_server_in_db(request)
        .await
        .map_err(|e| StatusCode::INTERNAL_SERVER_ERROR);
    result
}

async fn get_integrations(State(arc_state): State<Arc<RwLock<AppState>>>) -> impl IntoResponse {
    let state = arc_state.write().await;

    let result = state
        .database
        .fetch_all_intergrations()
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR);

    match result {
        Ok(intergrations) => (
            StatusCode::OK,
            Json(List {
                list: ApiCalls::IntergrationsDataList(intergrations),
            }),
        )
            .into_response(),

        Err(status) => status.into_response(),
    }
}

//modify_intergration
async fn modify_intergration(
    State(arc_state): State<Arc<RwLock<AppState>>>,
    Json(request): Json<ModifyElementData>,
) -> impl IntoResponse {
    let state = arc_state.write().await;

    //let mut fetched_intergration: Option<Result<Option<Intergration>, StatusCode>>;
    if let Element::Intergration(ref intergration_element) = request.element {
        let fetched_intergration_result = state
            .database
            .get_from_intergrations_database(&intergration_element.r#type.to_string())
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR);
        if let Ok(Some(final_intergration)) = fetched_intergration_result {
            // if final_intergration.settings.get("enable_")
            if let Some(settings) = final_intergration.settings.as_object() {
                let enabled_keys: Vec<&String> = settings
                    .iter()
                    .filter(|(key, _)| key.starts_with("enable"))
                    .filter_map(|(key, value)| {
                        if let Value::Bool(b) = value {
                            if *b == false { Some(key) } else { None }
                        } else {
                            None
                        }
                    })
                    //.filter(|(_, value)| matches!(value, Value::Bool(_)))
                    //.map(|(key, _)| key)
                    .collect();

                for enabled_key in enabled_keys {
                    let hook = settings
                        .iter()
                        .find(|(key, _)| key.starts_with(&format!("_{}_hook", enabled_key)));
                    //println!("{:#?}", hook);
                    if let Some(unwrapped_hook) = hook {
                        if let Some(Value::Bool(new_enabled_key)) =
                            intergration_element.settings.get(enabled_key)
                        {
                            if *new_enabled_key == true {
                                // println!("unwrapped hook {:#?}", unwrapped_hook.1);
                                match serde_json::to_vec(&unwrapped_hook.1) {
                                    Ok(mut bytes) => {
                                        // Add newline delimiter for TCP stream parsing
                                        bytes.push(b'\n');

                                        // Send to internal_tx for local processing
                                        // This goes to internal_stream in handle_stream
                                        if let Some(ref internal_tx) = state.internal_tx {
                                            if let Err(err) = internal_tx.send(bytes.clone()) {
                                                eprintln!(
                                                    "Failed to send to internal stream: {}",
                                                    err
                                                );
                                            }
                                        }

                                        // // Send to tcp_tx to forward to remote server
                                        // if let Err(err) = state.tcp_tx.send(bytes) {
                                        //     eprintln!("Failed to send to TCP stream: {}", err);
                                        // }
                                    }
                                    Err(err) => eprintln!("Failed to serialize request: {}", err),
                                }
                                // let maybe_command: Result<IntergrationCommands, _> = serde_json::from_value(unwrapped_hook.1.clone());
                                // if let Ok(command) = maybe_command {
                                //     run_intergration_commands(command)
                                // }
                                // if let IntergrationCommands = unwrapped_hook.1 {
                                //     println!("{}", command);
                                // }
                            }
                        }
                    }
                }
                // for (key, value) in settings {
                //    if key.starts_with("enable"){

                //    }
                // }
            }
        }
    } else {
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    }

    match state.database.edit_intergrations_in_db(request).await {
        Ok(status_code) => (
            status_code,
            Json(serde_json::json!({
                "success": true,
                "message": "Integration modified successfully"
            })),
        )
            .into_response(),
        Err(e) => {
            //eprintln!("Database error: {:#?}", e);

            let status_code = if let Some(db_err) = e.downcast_ref::<DatabaseError>() {
                db_err.0
            } else {
                StatusCode::INTERNAL_SERVER_ERROR
            };

            let error_message = match status_code {
                StatusCode::NOT_FOUND => "Integration not found",
                StatusCode::BAD_REQUEST => "Invalid request data",
                _ => "Internal server error",
            };

            (
                status_code,
                Json(serde_json::json!({
                    "success": false,
                    "error": error_message
                })),
            )
                .into_response()
        }
    }
}

async fn delete_intergration(
    State(arc_state): State<Arc<RwLock<AppState>>>,
    Json(request): Json<ModifyElementData>,
) -> impl IntoResponse {
    let state = arc_state.write().await;
    let result = state
        .database
        .remove_intergrations_in_db(request)
        .await
        .map_err(|e| {
            println!("{:#?}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        });
    result
}
async fn create_intergration(
    State(arc_state): State<Arc<RwLock<AppState>>>,
    Json(request): Json<ModifyElementData>,
) -> impl IntoResponse {
    //println!("Received request: {:#?}", request);
    let state = arc_state.write().await;

    match state.database.create_intergrations_in_db(request).await {
        Ok(status_code) => (
            status_code,
            Json(serde_json::json!({
                "success": true,
                "message": "Integration created successfully"
            })),
        )
            .into_response(),
        Err(e) => {
            // eprintln!("Database error: {:#?}", e);
            // eprintln!("Error source: {:?}", e.source());

            let status_code = if let Some(db_err) = e.downcast_ref::<DatabaseError>() {
                db_err.0
            } else {
                StatusCode::INTERNAL_SERVER_ERROR
            };

            let error_message = match status_code {
                StatusCode::CONFLICT => "Integration already exists",
                StatusCode::BAD_REQUEST => "Invalid request data",
                _ => "Internal server error",
            };

            (
                status_code,
                Json(serde_json::json!({
                    "success": false,
                    "error": error_message
                })),
            )
                .into_response()
        }
    }
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

// This sets the current server
async fn set_server(
    State(arc_state): State<Arc<RwLock<AppState>>>,
    Json(request): Json<ModifyElementData>,
) -> Result<StatusCode, StatusCode> {
    let mut state = arc_state.write().await;
    if let Element::String(servername) = request.element {
        // its unusual for two ?? but it works
        let retrieved_server = state
            .database
            .get_from_servers_database(&servername)
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
            .transpose()
            .ok_or(StatusCode::NOT_FOUND)??;

        state.current_server = Some(servername);

        let msg = MessagePayloadWithMetadata {
            r#type: "command".to_string(),
            message: "set_server".to_string(),
            metadata: MetadataTypes::Server {
                servername: retrieved_server.servername,
                provider: retrieved_server.provider,
                providertype: retrieved_server.providertype,
                location: retrieved_server.location,
            },
            authcode: "0".to_string(),
        };

        let mut bytes = serde_json::to_vec(&msg).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        bytes.push(b'\n');
        state.tcp_tx.send(bytes).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

        Ok(StatusCode::OK)
    } else {
        Ok(StatusCode::INTERNAL_SERVER_ERROR)
    }
}

// async fn set_server(
//     State(arc_state): State<Arc<RwLock<AppState>>>,
//     Json(request): Json<ModifyElementData>,
// ) -> impl IntoResponse {
//     let state = arc_state.write().await;
//     if let Element::Server(server) = request.element {
//         //if state.database.get_from_servers_database(&server.servername).await.ok().is_some() {
//         if let Ok(retrived_server_option) = state
//             .database
//             .get_from_servers_database(&server.servername)
//             .await
//         {
//         if let Some(retrieved_server) = retrived_server_option {
//                 let msg = MessagePayloadWithMetadata {
//                     r#type: "command".to_string(),
//                     message: "set_server".to_string(),
//                     metadata: MetadataTypes::Server {
//                         servername: retrieved_server.servername,
//                         provider: retrieved_server.provider,
//                         providertype: retrieved_server.providertype,
//                         location: retrieved_server.location,
//                     },
//                     authcode: "0".to_string(),
//                 };

//                 match serde_json::to_vec(&msg) {
//                     Ok(mut bytes) => {
//                         bytes.push(b'\n');
//                         if let Err(err) = state.tcp_tx.send(bytes) {
//                             eprintln!("Failed to send over TCP: {}", err);
//                             return StatusCode::INTERNAL_SERVER_ERROR;
//                         }
//                     }
//                     Err(err) => {
//                         eprintln!("Failed to serialize: {}", err);
//                         return StatusCode::INTERNAL_SERVER_ERROR;
//                     }
//                 }
//                 StatusCode::OK
//             } else {
//                 StatusCode::INTERNAL_SERVER_ERROR
//             }
//         } else {
//             StatusCode::INTERNAL_SERVER_ERROR
//         }
//         // if let Ok(servers) = state.database.fetch_all_servers().await {
//         //     state.server = server;
//         //     StatusCode::Ok
//         // } else {
//         //     StatusCode::INTERNAL_SERVER_ERROR
//         // }
//     } else {
//         StatusCode::INTERNAL_SERVER_ERROR
//     }
//     // let result = state
//     //     .database
//     //     .edit_user_in_db(request)
//     //     .await
//     //     .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR);
//     // result
// }

// gets the server from the database, if the incoming request is empty, it will give the current server
async fn get_server(
    State(arc_state): State<Arc<RwLock<AppState>>>,
    Json(request): Json<RetrieveElement>,
) -> Result<Json<Server>, StatusCode> {
    let state = arc_state.write().await;
    //if let Some(unwrapped_current_server) = &state.current_server {
    //let current_server = state.database.get_settings().await.
    let mut server_to_get = request.element.clone();
    if state.current_server.is_some() && request.element.is_empty() {
        server_to_get = state.current_server.clone().unwrap();
        // let updated_current_server = 
        //     state.database.get_from_servers_database(&state.current_server.clone().unwrap())
        //         .await
        //         .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
        //         .transpose()
        //         .ok_or(StatusCode::NOT_FOUND)??;               
        //Ok(Json(updated_current_server))
    } else {}

    // A bit unusual to have two ?? but it works in this case
    let result = state
        .database
        .get_from_servers_database(&server_to_get)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
        .transpose()
        .ok_or(StatusCode::NOT_FOUND)??;
    Ok(Json(result))
}

// get the user from the db
async fn get_user(
    State(arc_state): State<Arc<RwLock<AppState>>>,
    Json(request): Json<RetrieveElement>,
) -> impl IntoResponse {
    let state = arc_state.write().await;
    let result = state
        .database
        .get_from_database(&request.element)
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

// Capabilities (in this function) notifies the frontend if th backend has certain things enabled, like a
// samba server and etc
async fn capabilities(State(arc_state): State<Arc<RwLock<AppState>>>) -> impl IntoResponse {
    let state = arc_state.write().await;
    let mut capabilities: Vec<String> = vec![];
    capabilities.push("all".to_string());
    // capabilities.push("all");
    Json({ capabilities }).into_response()
}

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
//process_general_with_metadata

// as I said in main, this is temporary, because IncomingMessageWithMetadata is supposed to replace IncomingMessage
// but for now I will have it as a seprate route for new routes which need a metadata feild until a replace everything
// or ill remove both in favor of individual routes
async fn process_general_with_metadata(
    State(arc_state): State<Arc<RwLock<AppState>>>,
    Json(res): Json<ApiCalls>,
) -> Result<Json<ResponseMessage>, (StatusCode, String)> {
    let state = arc_state.write().await;
    let database = &state.database;
    if let ApiCalls::IncomingMessageWithMetadata(payload) = res {
        println!("Processing general message: {:?}", payload);

        let json_payload = IncomingMessageWithMetadata {
            message_type: payload.message_type.clone(),
            message: payload.message.clone(),
            authcode: payload.authcode.clone(),
            metadata: payload.metadata.clone(),
        };

        if payload.message == "create_server" {
            let db_result = database
                .get_from_servers_database(&payload.message.clone())
                .await
                .map_err(|_| {
                    (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        "Error connecting to database".to_string(),
                    )
                });
            if db_result?.is_none() {
                if let MetadataTypes::Server {
                    providertype,
                    location,
                    provider,
                    servername,
                } = payload.metadata.clone()
                {
                    let _ = database
                        .create_server_in_db(ModifyElementData {
                            element: Element::Server(Server {
                                servername: payload.message.clone(),
                                provider: String::new(),
                                providertype,
                                location,
                            }),
                            jwt: payload.authcode,
                            require_auth: false,
                        })
                        .await;
                }
            }
        }

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
        list: ApiCalls::UserDataList(users),
    }))
}
async fn change_node(
    State(arc_state): State<Arc<RwLock<AppState>>>,
    Json(request): Json<IncomingMessage>,
) -> Result<StatusCode, StatusCode> {
    let option_node = {
        let state = arc_state.read().await;
        state.database.retrieve_nodes(request.message).await
    };

    if let Some(node) = option_node {
        {
            let termination_payload = MessagePayload {
                r#type: "conn_state".to_string(),
                message: "end_conn".to_string(),
                authcode: "0".to_string(),
            };

            let termination_bytes = serde_json::to_vec(&termination_payload).unwrap_or_default();

            if let Some(tx) = &arc_state.read().await.internal_tx {
                let _ = tx.send(termination_bytes);
            }
        }

        let (new_tcp_tx, new_tcp_rx) = broadcast::channel::<Vec<u8>>(100);
        let (internal_tx, internal_rx) = broadcast::channel::<Vec<u8>>(100);

        {
            let mut state = arc_state.write().await;
            state.tcp_tx = new_tcp_tx.clone();
            state.tcp_rx = new_tcp_rx.resubscribe();
            state.internal_tx = Some(internal_tx);
            state.internal_rx = Some(internal_rx.resubscribe());
        }

        let bridge_rx = new_tcp_tx.subscribe();
        let bridge_tx = arc_state.read().await.ws_tx.clone();
        let internal_stream = Some(internal_rx);

        if let Err(e) = connect_to_server(
            Arc::clone(&arc_state),
            node.ip.to_string(),
            bridge_rx,
            bridge_tx,
            internal_stream,
            true,
            false,
        )
        .await
        {
            eprintln!("Connection task failed: {}", e);
        } else {
            let mut state = arc_state.write().await;
            state.current_node = NodeAndTCP {
                name: node.nodename,
                ip: node.ip,
                ..Default::default()
            };

            let node_state_payload = MessagePayload {
                r#type: "command".to_string(),
                message: "server_state".to_string(),
                authcode: "0".to_string(),
            };
            let node_state_bytes = serde_json::to_vec(&node_state_payload).unwrap_or_default();

            if let Some(tx) = &state.internal_tx {
                let _ = tx.send(node_state_bytes);
            }
        }

        Ok(StatusCode::OK)
    } else {
        println!("Error: node not found");
        Err(StatusCode::INTERNAL_SERVER_ERROR)
    }
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

    if let Clients::K8s(client) = state.client.clone() {
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
                    ..Default::default()
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

    //let node_name_list: Vec<String> = node_list.into_iter().map(|node| node.name).collect();
    let regular_node_list: Vec<Node> = node_list
        .into_iter()
        .map(|node_and_tcp| Node {
            nodename: node_and_tcp.name,
            ip: node_and_tcp.ip,
            nodestatus: NodeStatus::Unknown,
            nodetype: node_and_tcp.nodetype,
            k8s_type: node_and_tcp.k8s_type,
        })
        .collect();

    Json(List {
        list: ApiCalls::NodeDataList(regular_node_list),
    })
}

async fn get_servers(
    State(arc_state): State<Arc<RwLock<AppState>>>,
) -> Result<Json<List>, StatusCode> {
    let mut state = arc_state.write().await;
    let result = match state.database.fetch_all_servers().await {
        Ok(servers) => Ok(Json(List {
            list: ApiCalls::ServerDataList(servers),
        })),
        Err(_) => return Err(StatusCode::INTERNAL_SERVER_ERROR),
    };
    //println!("{:#?}", result);
    result
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
    let path = Path::new("src/frontend/build").join(file);

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
        list: ApiCalls::FileDataList(items),
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

pub async fn stream_file_download(
    State(state): State<Arc<RwLock<AppState>>>,
    axum::extract::Path(file_path): axum::extract::Path<String>,
) -> Result<Response<Body>, StatusCode> {
    let tcp_fs = {
        let state = state.read().await;
        let (tcp_tx, tcp_rx) = (state.tcp_tx.clone(), state.tcp_tx.subscribe());
        Arc::new(Mutex::new(TcpFs::new(tcp_tx, tcp_rx)))
    };

    let decoded_path = urlencoding::decode(&file_path)
        .map_err(|_| StatusCode::BAD_REQUEST)?
        .to_string();

    let normalized_path = normalize_and_secure_path(&decoded_path)?;

    let metadata = {
        let mut fs = tcp_fs.lock().await;
        let mut remote_fs = RemoteFileSystem::new(&normalized_path, Some((*fs).clone()));

        let is_file = remote_fs.is_file().await.map_err(|e| {
            eprintln!("Error checking if path is file: {}", e);
            StatusCode::NOT_FOUND
        })?;

        if !is_file {
            return Err(StatusCode::BAD_REQUEST);
        }

        remote_fs.ensure_metadata().await.map_err(|e| {
            eprintln!("Error getting metadata: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

        remote_fs.cached_metadata.clone()
    };

    let file_size = None;
    let chunk_size = 64 * 1024;

    let stream = TcpFileStream::new(
        tcp_fs.clone(),
        normalized_path.clone(),
        file_size,
        chunk_size,
    );

    let body = Body::from_stream(stream);

    let filename = std::path::Path::new(&normalized_path)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("download");

    let mut response = Response::new(body);
    let headers = response.headers_mut();

    headers.insert(
        header::CONTENT_TYPE,
        "application/octet-stream".parse().unwrap(),
    );
    headers.insert(
        header::CONTENT_DISPOSITION,
        format!("attachment; filename=\"{}\"", filename)
            .parse()
            .unwrap(),
    );

    if let Some(size) = file_size {
        headers.insert(header::CONTENT_LENGTH, size.to_string().parse().unwrap());
    } else {
        headers.insert(header::TRANSFER_ENCODING, "chunked".parse().unwrap());
    }

    Ok(response)
}
fn normalize_and_secure_path(path: &str) -> Result<String, StatusCode> {
    use std::path::{Component, Path, PathBuf};

    let path = path.trim();

    let path_buf = PathBuf::from(path);

    let mut normalized = PathBuf::new();
    for component in path_buf.components() {
        match component {
            Component::Normal(part) => normalized.push(part),
            Component::RootDir => {
                continue;
            }
            Component::CurDir => {
                continue;
            }
            Component::ParentDir => {
                if normalized.components().count() > 0 {
                    normalized.pop();
                }
            }
            _ => {
                return Err(StatusCode::BAD_REQUEST);
            }
        }
    }

    let normalized_str = normalized.to_string_lossy().to_string();

    let server_prefix = "server/";
    let final_path = if normalized_str.starts_with(server_prefix) {
        normalized_str
    } else if normalized_str.is_empty() {
        return Err(StatusCode::BAD_REQUEST);
    } else {
        format!("{}{}", server_prefix, normalized_str)
    };

    if !final_path.starts_with(server_prefix) {
        eprintln!("Path traversal attempt blocked: {} -> {}", path, final_path);
        return Err(StatusCode::FORBIDDEN);
    }

    if final_path.contains('\0') {
        eprintln!("Null byte in path blocked: {}", final_path);
        return Err(StatusCode::BAD_REQUEST);
    }

    Ok(final_path)
}

// Unit tests
#[cfg(test)]
mod tests {
    use super::*;

    mod db_users {
        use super::*;
        use crate::database::{Database, Element};

        #[cfg(all(
            not(feature = "full-stack"),
            not(feature = "docker"),
            not(feature = "database")
        ))]
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
                    require_auth: false,
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
