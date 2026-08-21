// ALOT of imports, needed given the size of this project in what it covers
// first imports are std ones
use std::collections::HashMap;
use std::convert::Infallible;
use std::fmt::Debug;
use std::{net::SocketAddr, path::Path, sync::Arc};

use crate::database::Database;
// Axum is the routing framework, and the backbone to this project helping intergrate the backend with the frontend
// and the general api, redirections, it will take form data and queries and make it easily accessible
// I also use axum_login to take off alot of effort that would be required for authentication
use crate::database::databasespec::{
    Filters, IntoServer, ServerMetadata, resolve_database_error_into_statuscode,
};
use crate::database::{DatabaseError, Element};
use crate::docker::BuildImageRequest;
use crate::filesystem::FileSystemHandler;
// use crate::filesystem::{execute_file_operation, FileOperations, TcpFileStream};
// use crate::filesystem::{FsType, send_multipart_over_broadcast};
use crate::http::HeaderMap;
use crate::kubernetes::{BuildDeploymentRequest, ListNodeInfoRequest};
use crate::middleware::from_fn;
use axum::Form;
use axum::error_handling::HandleErrorLayer;
use axum::extract::Multipart;
use axum::middleware::{self, Next};
use axum::response::Redirect;
use axum::response::Response;
use axum::routing::any;
use axum::routing::delete;
use axum::{
    Router,
    body::Body,
    extract::{
        Request, State,
        ws::{Message, WebSocket, WebSocketUpgrade},
    },
    http::{self, Method, StatusCode},
    response::{
        Html, IntoResponse, Json,
        sse::{Event, Sse},
    },
    routing::{get, post, put},
};
use axum_login::AuthManagerLayer;
use axum_login::AuthUser;
use axum_login::tower_sessions::{MemoryStore, SessionManagerLayer};
use axum_login::{AuthManagerLayerBuilder, AuthnBackend};
use axum_oidc::OidcAuthLayer;
use axum_oidc::OidcClaims;
use axum_oidc::OidcClient;
use axum_oidc::OidcLoginLayer;
use axum_oidc::error::MiddlewareError;
use axum_oidc::handle_oidc_redirect;
use axum_oidc::openidconnect::ClientId;
use axum_oidc::openidconnect::ClientSecret;
use axum_oidc::openidconnect::IssuerUrl;
use axum_oidc::openidconnect::Scope;
use general_networked_filesystem::flume_delimited::{FlumeFile, TcpFsReceiver, TcpFsSender};
use general_networked_filesystem::{FileOperations, LsRequest, RemoteFileSystem};

use tokio::sync::{watch, RwLock};

use rcon::Connection;
use tokio_util::sync::CancellationToken;
use tower::ServiceBuilder;

use crate::database::Node;
use crate::database::databasespec::Intergration;
use crate::database::databasespec::NodeType;
use crate::database::databasespec::NodesDatabase;
use crate::database::databasespec::UserDatabase;
use crate::database::databasespec::UserPerm;
use crate::database::databasespec::{Button, NodeStatus};
use crate::database::databasespec::{
    ButtonsDatabase, IntergrationsDatabase, K8sType, Server, ServerDatabase, Settings,
    SettingsDatabase,
};

// miscellancious imports, future traits are used because alot of the code is asyncronus and cant fully be contained in tokio
// mime_guess as when I am serving the files, I need to serve it with the correct mime type
// serde_json because I exchange alot of json data between the backend and frontend and to the gameserver
// tokio because when working with alot of networking stuff and things that will take a indeterminent amount of time, async/await is the way to go (for better efficency too)
// chrono for time, tower for cors (TODO:: use less permissive CORS due to potential security risks)
// jsonwebtokens is standard when working with authentication, and bcrypt so I can use password hashs, I explain the authentication methods later
use async_trait::async_trait;
use futures_util::{sink::SinkExt, stream::StreamExt};
use jsonwebtoken::{DecodingKey, TokenData, Validation, decode};
use mime_guess::from_path;
// use serde;
use futures_util::{Stream, stream};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::time::{interval};
use tokio::{
    fs as tokio_fs,
    net::{TcpListener, TcpStream},
    sync::{Mutex, broadcast},
    time::{Duration},
};
use tokio::sync::Notify;
use tower_http::cors::{Any as CorsAny, CorsLayer};

use std::error::Error;
use std::sync::atomic::{AtomicUsize, Ordering};

use sysinfo::System;

static CONNECTION_COUNTER: AtomicUsize = AtomicUsize::new(0);

// For now I only restrict the json backend for running this without kubernetes
// the json backend is only for testing in most cases, simple deployments would use full-stack feature flag
// and you can use postgres manually with the database feature flag
#[cfg(any(feature = "full-stack", feature = "database"))]
mod database {
    include!("pgdatabase.rs");
}
// TODO: consider if i want to have a varible that represents the database type enabled
// static DATABASE_TYPE = "postgres";

#[cfg(all(not(feature = "full-stack"), not(feature = "database")))]
mod database {
    include!("jsondatabase.rs");
}

// JsonDatabase is only something that would be unique to Json and not any other database managed by sqlx
#[cfg(all(not(feature = "full-stack"), not(feature = "database")))]
use database::JsonBackend;

// Both database files and any more should have these structs
use crate::database::databasespec::RetrieveElement;
use database::ModifyElementData;
use database::User;

// #[cfg(not(feature = "grpc_experimental"))]
// mod transport;

// #[cfg(feature = "grpc_experimental")]
// mod transport;

mod transport;
mod filesystem;

use crate::transport::node_transport::ConnectionHandler;
use crate::transport::node_transport::try_initial_connection;
use crate::transport::node_transport::{
    CreateServerRequest, DeleteServerRequest, FilterRequest,
    IntegrationKeyRequest, MigrateRequest, NodeTransportable, StreamTransportable,
    Ping, ServerDataRequest, 
    SetServerRequest, StartServerRequest, StopServerRequest, check_channel_health,
    connect_to_server,
};

mod extra;

// // Docker AND kubernetes would be enabled with a standard deployment
// // as you wouldnt need the docker module (or the k8s module) for barebones testing
#[cfg(feature = "full-stack")]
mod orchestrator;

#[cfg(feature = "full-stack")]
pub use orchestrator::docker;

#[cfg(feature = "full-stack")]
pub use orchestrator::kubernetes;

#[cfg(feature = "full-stack")]
pub use kubernetes::KubeLocalRequest;

#[cfg(feature = "full-stack")]
pub use docker::DockerLocalRequest;

// Main has to store the client, so I would remove the client here if this is not in a standard deployment in favor
// of a dummy one
// #[cfg(feature = "full-stack")]
// use k8s_orchestrator::{Client};
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
    use crate::NodeWithStream;

    pub async fn create_k8s_deployment(
        _: &crate::Client,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        Err("This should not be running".into())
    }
    #[allow(unused)]
    pub async fn list_node_names(
        _: crate::Client,
    ) -> Result<Vec<String>, Box<dyn std::error::Error>> {
        Err("This should not be running".into())
    }
    pub async fn list_node_info(
        _: crate::Client,
    ) -> Result<Vec<NodeWithStream>, Box<dyn std::error::Error + Send + Sync>> {
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
        _client: &crate::Client,
        _ip: String,
    ) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
        Ok(false)
    }
    pub async fn verify_is_k8s_node(
        _client: &crate::Client,
        _ip: String,
    ) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
        Ok(false)
    }
}

// This part would potentially be removed later
// I like these defaults for testing, and for the moment I doubt anyone would object
// but at some point this will be removed in favor of testing with ENV varibles
#[cfg(not(feature = "full-stack"))]
static STATIC_NODE_URL: &str = "127.0.0.1:8082";

#[cfg(not(feature = "full-stack"))]
static STATIC_LOCAL_URL: &str = "127.0.0.1:8083";

#[cfg(not(feature = "full-stack"))]
static K8S_WORKS: bool = false;

// TODO: consider having some feature thats aware if docker is avalible or not
#[cfg(not(feature = "docker"))]
static DOCKER_WORKS: bool = false;

#[cfg(feature = "full-stack")]
static STATIC_NODE_URL: &str = "gameserver-service:8080";

#[cfg(feature = "full-stack")]
static STATIC_LOCAL_URL: &str = "127.0.0.1:8080";

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
#[cfg(any(feature = "full-stack", feature = "database"))]
async fn first_connection() -> Result<sqlx::Pool<sqlx::Postgres>, sqlx::Error> {
    // TODO: use get_env_or_arg here?
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

// For very simple messages like pings that need no added complexity
#[derive(Debug, serde::Serialize, serde::Deserialize)]
struct SimpleMessage {
    message: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct SimpleMesagePayload {
    message: String,
    authcode: String,
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

// TODO: Is this truely nessesary for the RCON response?
// Could I use SimpleMessage instead?
#[derive(Debug, Serialize, Deserialize)]
struct UnauthenticatedMessagePayload {
    r#type: String,
    message: String,
}

pub enum StreamResult {
    Done,
    Reconnect(String, String),
}

// #[derive(PartialEq)]
#[derive(Default)]
pub struct NodeWithStream {
    name: String,
    ip: String,
    status: Status,
    nodetype: NodeType,
    k8s_type: K8sType,
    gameserver: Value,
    tx: Option<tokio::sync::broadcast::Sender<Vec<u8>>>,
    rx: Option<tokio::sync::broadcast::Receiver<Vec<u8>>>,
}
impl Clone for NodeWithStream {
    fn clone(&self) -> NodeWithStream {
        NodeWithStream {
            name: self.name.clone(),
            ip: self.ip.clone(),
            nodetype: self.nodetype.clone(),
            status: self.status.clone(),
            gameserver: self.gameserver.clone(),
            tx: self.tx.clone(),
            rx: self.tx.as_ref().map(|tx| tx.subscribe()),
            k8s_type: self.k8s_type.clone(),
        }
    }
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
        sandbox: bool,
        server_metadata: ServerMetadata,
    },
    Filter(Filters),
    DeleteServerFiles(bool),
    DeleteServer {
        delete_server_name: String,
        delete_server_files: bool,
    },
    // TODO: remove these types in favor of explicit handling
    String(String),
    Boolean(bool),
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(tag = "kind", content = "data")]
enum IntegrationCommands {
    MinecraftEnableRcon(serde_json::Value),
    MinecraftDisableRcon(serde_json::Value),
}

#[derive(Clone)]
pub struct K8sClient {
    k8s_client: Client,
    docker_info: String,
}

#[derive(Clone)]
struct K8sGrpcClients {

}

#[derive(Clone)]
enum Clients {
    K8sLocal(K8sClient),
    K8sRemote(K8sGrpcClients),
    Docker(String),
    None,
}

// console output is sometimes contained within the data feild of json, but this also might be redundant
#[derive(Debug, Deserialize, Serialize)]
pub struct InnerData {
    data: String,
    #[serde(rename = "type")]
    message_type: String,
}

#[derive(Debug, Serialize)]
pub struct SignInResponse {
    username: String,
}

#[derive(Debug, Deserialize, Serialize)]
struct Statistics {
    used_memory: u64,
    total_memory: u64,
    core_data: Vec<f32>,
    metadata: String,
}
// a list for things like nodes, capabilities, etc
#[derive(Debug, Serialize, Deserialize)]
pub struct List {
    list: ApiCalls,
}

// May be redundant, but this is a struct for incoming messages
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct IncomingMessage {
    message: String,
    #[serde(rename = "type")]
    message_type: String,
    authcode: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct AuthTcpMessage {
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

#[derive(Debug, Deserialize, Serialize, Clone)]
struct OidcAdditionalClaims {
    #[serde(default)]
    user_perms: Option<Vec<UserPerm>>,
}

impl axum_oidc::openidconnect::AdditionalClaims for OidcAdditionalClaims {}
impl axum_oidc::AdditionalClaims for OidcAdditionalClaims {}

// Some common api calls which is just what might get exchanged between the frontend and backend via api
// this is needed rather than a bunch of structs or however else I might do it because in some cases I might not know what api call to expect
// as it would be determined by a 'kind' flag provided by serde, and the content, be it a array or struct, be nested in json (which provides new hurdles for how to process
// data as I cant EXPECT JSON in there)
// TODO: phase out, this works on the outdated model where i send everything in one route which is automatically proxied
// to the node, but this should not always be the case, atleast this was the primary use
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
    // FileDataList(Vec<FsItem>),
    Node(Node),
    //FileOperations(FileOperations), // FileMoveOperation(String),
    // FileCopyOperation(String),
    // FileZipOperation(String),
    // FileUnzipOperation(String),
    // FileDownloadOperation(String),
    // FileDownloadAllOperation(String),
    // FileUploadAllOperation(String),
}

// impl fmt::Display for ApiCalls {
//     fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
//         let s = match self {
//             ApiCalls::FileDownloadOperation(_) => "FileDownloadOperation",
//             ApiCalls::FileZipOperation(_) => "FileZipOperation",
//             ApiCalls::FileMoveOperation(_) => "FileMoveOperation",
//             ApiCalls::FileUnzipOperation(_) => "FileUnzipOperation",
//             ApiCalls::FileCopyOperation(_) => "FileCopyOperation",
//             ApiCalls::FileUploadAllOperation(_) => "FileUploadAllOperation",
//             ApiCalls::FileDownloadAllOperation(_) => "FileDownloadAllOperation",
//             _ => "not implemented",
//         };
//         write!(f, "{}", s)
//     }
// }

// TODO: consider using this or implimenting a broader struct for any object
// struct ApiCallsWithAuth {
//     jwt: String,
//     require_auth: String,
//     api_call: ApiCalls
// }
#[derive(Clone, Default, Serialize, Deserialize, Debug)]
pub enum Status {
    Unknown,
    Up,
    Healthy,
    #[default]
    Down,
    Unhealthy,
}


// AppState, this is a global struct which will be used to store data needed across the application like in routes and etc
// which includes the sender and reciver to the tcp connection for gameserver, the websocket sender (receiver only needs to be managed by its own handler)
// the base path like if all the routes are prefixed with something like /gameserver-rs which is the default for my testing deployment, and database as its needed frequently
// for user information and etc
// #[derive(Default)]
pub struct AppState {
    // tx: tokio::sync::broadcast::Sender<Vec<u8>>,
    // rx: tokio::sync::broadcast::Receiver<Vec<u8>>,
    connection_handler: ConnectionHandler,
    cancel_current_conn: CancellationToken,
    conn_status: Status,
    internal_rx: Option<broadcast::Receiver<Vec<u8>>>,
    internal_tx: Option<broadcast::Sender<Vec<u8>>>,
    additonal_node: Vec<NodeWithStream>,
    current_node: NodeWithStream,
    ws_tx: broadcast::Sender<String>,
    server_start_event: Arc<Notify>,
    //ws_rx: broadcast::Receiver<String>,
    server_console: Option<broadcast::Sender<String>>,
    base_path: String,
    client: Clients,
    database: database::Database,
    cached_status_type: watch::Sender<String>,
    poll_server_event: Arc<Notify>,
    rcon_connection: Option<Arc<Mutex<Connection<TcpStream>>>>,
    current_server: Option<Server>,
    lock: bool,
    filesystem: FileSystemHandler, // filesystem: Option<RemoteFileSystem<TcpFs>>
}



// Looks for a env varible, if its not found, try the specified default, if none is found it will use the default of whatever that type is
fn get_env_var_or_arg<T>(env_var: &str, default: Option<T>) -> Option<T>
where
    T: std::str::FromStr + Clone,
{
    env::var(env_var)
        .ok()
        .and_then(|s| s.parse::<T>().ok())
        .or(default)
}

async fn ensure_admin_user(database: Database) {
    let enable_admin_user = std::env::var("ENABLE_ADMIN_USER").unwrap_or_default() == "true";
    let admin_user = std::env::var("ADMIN_USER").unwrap_or_default();
    let admin_password = std::env::var("ADMIN_PASSWORD").unwrap_or_default();
    if enable_admin_user {
        let _ = database
            .create_user_in_db(ModifyElementData {
                element: Element::User {
                    password: admin_password,
                    user: admin_user,
                    user_perms: vec![UserPerm {
                        perm: "admin".to_string(),
                        scope: "all".to_string(),
                    }],
                },
                jwt: "0".to_string(),
                require_auth: false,
            })
            .await;
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

    let database_conn_result = database.ensure_database_conn().await;

    ensure_admin_user(database.clone()).await;

    if let Err(err) = database_conn_result {
        println!("{}", err);
    }

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

    let config_node_url = get_env_var_or_arg("TCPURL", Some(STATIC_NODE_URL.to_string())).unwrap();
    let config_local_url =
        get_env_var_or_arg("LOCALURL", Some(STATIC_LOCAL_URL.to_string())).unwrap();

    // Overrides for testing or specific cases where how it works a setup may be diffrent
    let enable_k8s_client: bool = get_env_var_or_arg("ENABLE_K8S_CLIENT", Some(true)).unwrap();
    let force_rebuild: bool = get_env_var_or_arg("FORCE_REBUILD", Some(false)).unwrap();
    let build_docker_image: bool = get_env_var_or_arg("BUILD_DOCKER_IMAGE", Some(true)).unwrap();
    let build_deployment: bool = get_env_var_or_arg("BUILD_DEPLOYMENT", Some(true)).unwrap();
    let dont_override_conn_with_k8s: bool =
        get_env_var_or_arg("DONT_OVERRIDE_CONN_WITH_K8S", Some(true)).unwrap();

    // TODO:
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

    // sets the client to be none by default unless this is ran the stanard way which will be ran with the appropriate feature-flag
    // which will set the k8s client
    let mut client: Clients = Clients::None;
    if enable_k8s_client && K8S_WORKS {
        client = Clients::K8sLocal(K8sClient {
            k8s_client: Client::try_default().await?,
            docker_info: String::new()
        });
    }

    let mut node_url: String = config_node_url.to_string();
    if !dont_override_conn_with_k8s && let Clients::K8sLocal(ref inner_client) = client {
        let request = kubernetes::GetK8sGameserversRequest {
            // connection: inner_client.clone(),
        };
        if let Ok(Some(url_result)) = request.execute_locally(inner_client.clone()).await {
            node_url = url_result.clone();
        } else {
            println!(
                "Could not get a successful url for a existing gameserver, will try the fallback url"
            )    
        }
        // if let Ok(url_result) = &kubernetes::get_avalible_gameserver(&inner_client).await {
        //     node_url = url_result.clone();
        // } else {
        //     println!(
        //         "Could not get a successful url for a existing gameserver, will try the fallback url"
        //     )
        // }
    }

    let mut nodes: Vec<NodeWithStream> = vec![];
    if let Ok(db_nodes) = database.fetch_all_nodes().await {
        nodes = db_nodes
            .into_iter()
            .map(|node| NodeWithStream {
                name: node.nodename,
                nodetype: node.nodetype,
                ip: node.ip,
                ..Default::default()
            })
            .collect()
    }

    let (internal_tx, internal_rx) = broadcast::channel::<Vec<u8>>(100);

    let mut rcon_connection: Option<Arc<Mutex<Connection<TcpStream>>>> = None;
    if let Ok(retrived_db) = database.get_settings().await {
        if retrived_db.enabled_rcon {
            rcon_connection = match Connection::builder()
                .enable_minecraft_quirks(true)
                .connect(&retrived_db.rcon_url, &retrived_db.rcon_password)
                .await
            {
                Ok(conn) => Some(Arc::new(Mutex::new(conn))),
                Err(e) => {
                    eprintln!("Failed to connect to RCON: {}", e);
                    None
                }
            }
        }
    }

    let mut current_server = None;
    if !(database.get_settings().await?.current_server.into_server() == Server::default()) {
        current_server = Some(database.get_settings().await?.current_server.into_server())
    }

    let connection_handler = ConnectionHandler::new();

    let (fs_sender_tx, fs_sender_rx) = flume::unbounded();
    let (fs_receiver_tx, fs_receiver_rx) = flume::unbounded();

    let filesystem = FileSystemHandler::new(fs_sender_tx, fs_sender_rx, fs_receiver_tx, fs_receiver_rx);

    let cached_status_type = watch::channel(String::new()).0;

    // use everything so far to make the app state
    let mut state: AppState = AppState {
        // tx: tx,
        // rx: rx,
        connection_handler,
        cancel_current_conn: CancellationToken::new(),
        internal_rx: Some(internal_rx.resubscribe()),
        internal_tx: Some(internal_tx),
        ws_tx: ws_tx.clone(),
        // ws_rx: ws_rx.resubscribe(),
        server_console: None,
        server_start_event: Arc::new(Notify::new()),
        base_path: base_path.clone(),
        current_node: NodeWithStream::default(),
        database: database.clone(),
        client,
        additonal_node: nodes,
        conn_status: Status::Unknown,
        cached_status_type,
        poll_server_event: Arc::new(Notify::new()),
        rcon_connection,
        current_server,
        lock: false,
        filesystem,
    };
    state.conn_status = {
        if check_channel_health(&state).await {
            Status::Up
        } else {
            Status::Down
        }
    };

    let multifaceted_state = Arc::new(RwLock::new(state));
    let _ = load_settings(multifaceted_state.clone()).await;

    // CORS are currently very permissive
    let cors = CorsLayer::new()
        .allow_origin(CorsAny)
        .allow_methods([Method::GET, Method::POST])
        .allow_headers(CorsAny);

    let session_store = MemoryStore::default();
    // TODO:
    // In the future, once cookies work, improve overrall https support
    // there was an issue where users could not log in because cookies have the Secure flag
    // but the site is http, which causes the cookie to be blocked
    // meaning every request to protected routes had no session and redirected back to login
    let session_layer = SessionManagerLayer::new(session_store.clone())
        .with_secure(false)
        .with_same_site(tower_sessions::cookie::SameSite::Lax)
        .with_http_only(false)
        .with_path("/")
        .with_name("gameserver_session");

    let backend = Backend::new(database);
    let auth_layer: AuthManagerLayer<Backend, MemoryStore> =
        AuthManagerLayerBuilder::new(backend, session_layer.clone()).build();

    let (fallback_router, maybe_oidc_layer) =
        routes_static(multifaceted_state.clone(), auth_layer.clone()).await;

    // the main route, this serves all the api stuff that wont be behind a login, but I handle the main routes in routes_static for better control
    // over the authentication flow, if the api could be publically accessible in the future, you would need a diffrent way to authenticate with a api
    let inner = Router::new()
        .route("/api/nodes", get(get_nodes))
        .route("/api/buttons", get(get_buttons))
        .route("/api/servers", get(get_servers))
        .route("/api/users", get(users))
        .route("/api/ws", get(ws_handler))
        .route("/api/upload", post(upload))
        // .route("/api/download/{*wildcard}", get(stream_file_download))
        .route("/api/fileoperations", post(file_operations))
        .route("/api/statistics", get(statistics))
        .route("/api/getsettings", get(get_settings))
        .route("/api/awaitserverstatus", get(ongoing_server_status))
        .route("/api/intergrations", get(get_integrations))
        .route("/api/getcurrentnode", get(fetch_current_node))
        .route("/api/ping", post(ping))
        .route("/api/createintergrations", post(create_intergration))
        .route("/api/modifyintergrations", post(modify_intergration))
        .route("/api/deleteintergrations", post(delete_intergration))
        .route("/api/rconcommand", post(rcon_command))
        .route("/api/refreshstatus", post(refresh_status))
        .route("/api/setsettings", post(set_settings))
        .route("/api/changenode", put(change_node))
        .route("/api/migrate", post(migrate))
        .route("/api/getstatus", post(get_status))
        .route("/api/getfiles", post(get_files))
        .route("/api/getfilescontent", post(get_files_content))
        .route("/api/buttonreset", post(button_reset))
        .route("/api/editbuttons", post(edit_buttons))
        .route("/api/addnode", post(add_node))
        .route("/api/deletenode", post(delete_node))
        .route("/api/addserver", post(add_server))
        .route("/api/deleteserver", post(delete_server))
        .route("/api/startserver", post(start_server))
        .route("/api/stopserver", post(stop_server))
        .route("/api/setserver", post(set_server))
        .route("/api/getserver", post(get_server))
        .route("/api/edituser", post(edit_user))
        .route("/api/getuser", post(get_user))
        .route("/api/signin", post(sign_in))
        .route("/api/signout", delete(sign_out))
        .route("/api/user/me", get(user_me))
        .route("/api/createuser", post(create_user))
        .route("/api/deleteuser", post(delete_user))
        .route("/api/setlock", post(set_lock))
        .merge(fallback_router)
        .with_state(multifaceted_state.clone());

    let normal_routes = Router::new().merge(inner);

    let app = if base_path.is_empty() || base_path == "/" {
        let routed = normal_routes
            .layer(middleware::from_fn(
                |req: Request<Body>, next: Next| async move { next.run(req).await },
            ))
            .layer(cors)
            .layer(auth_layer);

        let routed = if let Some(oidc_layer) = maybe_oidc_layer {
            routed.layer(
                // Needs a service builder to convert a MiddleWareError into an actual reponse which can be
                // combined with the rest of the routes, this is also in static_routes
                ServiceBuilder::new()
                    .layer(HandleErrorLayer::new(|e: MiddlewareError| async move {
                        eprintln!("OIDC auth layer error: {e:#?}");
                        Redirect::to("/").into_response()
                    }))
                    .layer(oidc_layer),
            )
        } else {
            routed
        };
        routed.layer(session_layer)
    } else {
        let routed = Router::new()
            .nest(&base_path, normal_routes)
            .layer(middleware::from_fn(
                |req: Request<Body>, next: Next| async move {
                    eprintln!("INCOMING: {} {}", req.method(), req.uri());
                    next.run(req).await
                },
            ))
            .layer(cors)
            .layer(auth_layer);

        let routed = if let Some(oidc_layer) = maybe_oidc_layer {
            routed.layer(
                // Same reason for ServiceBuilder as explained above
                ServiceBuilder::new()
                    .layer(HandleErrorLayer::new(|e: MiddlewareError| async move {
                        eprintln!("OIDC auth layer error: {e:#?}");
                        Redirect::to("/oidc").into_response()
                    }))
                    .layer(oidc_layer),
            )
        } else {
            routed
        };

        // adds a session to everything
        routed.layer(session_layer)
    };

    // if there is supposed to be a initial connection and if there is a client (as it wont be able to create the deployment without it, and it would be pointless to create a docker container
    // without the abbility to deploy it)
    let inner_state = Arc::clone(&multifaceted_state);
    if enable_initial_connection {
        println!("Trying initial connection...");
        let state_clone = inner_state.clone();
        let ws_tx_clone = ws_tx.clone();
        let tx_clone = inner_state.write().await.connection_handler.tx.clone();
        let node_url_clone = node_url.to_string();

        // TODO: Since I never create a handler with initial connections, should i take it out of the thread?, or rather,
        // leave it to set a TcpStream for the appstate for when it sorts itself out
        tokio::spawn(async move {
            let initial_connection_result = try_initial_connection(
                initial_connection_attempts,
                initial_connection_timeout,
                false,
                &state_clone,
                node_url_clone,
                &ws_tx_clone,
                tx_clone,
            )
            .await;
            if initial_connection_result.is_err() {
                println!("All initial connections failed");
            }
            if initial_connection_result.is_err() || force_rebuild {
                if let Clients::K8sLocal(client) = &inner_state.write().await.client {
                    eprintln!(
                        "Initial connection failed or force rebuild enabled, will possibly enable auto-build (configurable)"
                    );
                    let mut unbuilt_img_was_the_issue = false;
                    if build_docker_image {
                        unbuilt_img_was_the_issue = true;
                        let request = BuildImageRequest { client: client.clone() };
                        if let Err(e) = request.execute_locally().await {
                            eprintln!("Failed to build docker image: {:#?}", e);
                        }
                        // if let Err(e) = docker::build_docker_image().await {
                        //     eprintln!("Failed to build docker image: {:#?}", e);
                        // }
                    }
                    if build_deployment {
                        unbuilt_img_was_the_issue = true;
                        let deployment = if std::env::var("TESTING").is_ok() {
                            println!("Using dev deployment");
                            "deployment-dev.yaml"
                        } else {
                            "deployment.yaml"
                        };


                        let request = BuildDeploymentRequest { 
                            // connection: client.clone(), 
                            deployment: deployment.to_string()
                        };
                        if let Err(e) = request.execute_locally(client.clone()).await {
                            eprintln!("Failed to create k8s deployment: {:#?}", e);
                        };
                        // if let Err(e) = kubernetes::create_k8s_deployment(&client).await {
                        //     eprintln!("Failed to create k8s deployment: {:#?}", e);
                        // }
                    }
                    if !unbuilt_img_was_the_issue {
                        // if let Some(initial_connection_result_string) = initial_connection_result.err().unwrap().to_string() {
                        if let Some(initial_connection_result_string) = initial_connection_result
                            .as_ref()
                            .err()
                            .unwrap()
                            .downcast_ref::<String>()
                        {
                            if !initial_connection_result_string.is_empty() {
                                println!("{:#?}", initial_connection_result_string);
                            }
                        } else {
                            println!("{:#?}", initial_connection_result.as_ref().err().unwrap());
                        }
                    }
                } else {
                    if let Some(initial_connection_result_string) = initial_connection_result
                        .as_ref()
                        .err()
                        .unwrap()
                        .downcast_ref::<String>()
                    {
                        if !initial_connection_result_string.is_empty() {
                            println!("{:#?}", initial_connection_result_string);
                        }
                    } else {
                        println!("{:#?}", initial_connection_result.as_ref().err().unwrap());
                    }
                }
            }
            // If the initial connection result succeeded, it will define all the relevent channels in AppState so messages can be sent
            // and recived from it, internal_tx will also be used as an internal messaging service which can be used
            // internal for things like terminating a connection to a node locally, or forwarding said message to node
            if initial_connection_result.is_ok() {
                println!("Creating a new connection");
                let (new_tx, new_rx) = broadcast::channel::<Vec<u8>>(100);
                let (internal_tx, internal_rx) = broadcast::channel::<Vec<u8>>(100);

                {
                    let mut state = inner_state.write().await;
                    state.connection_handler.tx = new_tx.clone();
                    state.connection_handler.rx = new_rx.resubscribe();
                    state.internal_tx = Some(internal_tx);
                    state.internal_rx = Some(internal_rx.resubscribe());
                }

                let bridge_tx = inner_state.read().await.ws_tx.clone();

                let connect_to_server_result =
                    connect_to_server(inner_state, node_url, bridge_tx, true).await;
                if let Err(_) = connect_to_server_result {
                    // println", err);
                    println!("got an error connecting to server");
                }
            }
        });
        // This will make sure that the results of the initial connections attempt to build a docker image, or create a k8s deployment succeeded
        // and it will log the result, will if its an error. An error not logging here does not mean the initial connection went fine
        // NOTE: we no longer await the handle here — the connection task runs in the background so axum::serve can start immediately
    }

    let addr: SocketAddr = config_local_url.parse().unwrap();
    println!("Listening on http://{}{}", addr, base_path);

    let listener = TcpListener::bind(addr).await?;
    axum::serve(listener, app.into_make_service()).await?;

    Ok(())
}

pub async fn set_lock(
    State(arc_state): State<Arc<RwLock<AppState>>>,
    auth_session: AuthSession,
    headers: HeaderMap,
    Json(request): Json<IncomingMessage>,
) -> impl IntoResponse {
    let mut state = arc_state.write().await;

    let authorized = authorize(&state, auth_session, headers, vec![]).await;
    if !authorized {
        return StatusCode::UNAUTHORIZED;
    }

    if let Ok(lock) = request.message.parse::<bool>() {
        state.lock = lock;
        StatusCode::CREATED
    } else {
        StatusCode::UNPROCESSABLE_ENTITY
    }
}



pub async fn stop_server(
    State(arc_state): State<Arc<RwLock<AppState>>>,
    auth_session: AuthSession,
    headers: HeaderMap,
) -> impl IntoResponse {
    let mut state = arc_state.write().await;

    let authorized = authorize(&state, auth_session, headers, vec!["manager".to_string()]).await;
    if !authorized {
        return StatusCode::UNAUTHORIZED.into_response();
    }

    let stop_server_request = StopServerRequest {};
    let _ = stop_server_request.node_transport(&mut state).await;

    StatusCode::CREATED.into_response()
}

pub async fn rcon_command(
    State(arc_state): State<Arc<RwLock<AppState>>>,
    auth_session: AuthSession,
    headers: HeaderMap,
    Json(request): Json<IncomingMessage>,
) -> impl IntoResponse {
    let state = arc_state.read().await;

    let authorized = authorize(&state, auth_session, headers, vec!["manager".to_string()]).await;
    if !authorized {
        return StatusCode::UNAUTHORIZED.into_response();
    }

    drop(state);

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

async fn file_operations(
    State(arc_state): State<Arc<RwLock<AppState>>>,
    auth_session: AuthSession,
    headers: HeaderMap,
    Json(request): Json<SrcAndDest>,
) -> StatusCode {
    let state = arc_state.write().await;

    let authorized = authorize(&state, auth_session, headers, vec!["manager".to_string()]).await;
    if !authorized {
        return StatusCode::UNAUTHORIZED;
    }

    let request_bytes = serde_json::to_vec(&request).unwrap_or_default();

    if let Some(tx) = &state.internal_tx {
        let _ = tx.send(request_bytes);
    }

    StatusCode::CREATED
}
#[axum::debug_handler]
async fn upload(
    State(arc_state): State<Arc<RwLock<AppState>>>,
    auth_session: AuthSession,
    headers: HeaderMap,
    // request: Request,
    mut multipart: Multipart,
) -> StatusCode {
    println!("got an upload request");
    let mut state = arc_state.write().await;
    let authorized = authorize(
        &state,
        auth_session,
        headers.clone(),
        vec!["manager".to_string()],
    )
    .await;
    if !authorized {
        return StatusCode::UNAUTHORIZED;
    }
    println!("passed auth");
    let (tx, rx) = flume::unbounded();
    state.filesystem.send_flume_file(None, "test.txt".to_string(), Some(rx));
    // let filesystem_sender: &mut RemoteFileSystem<TcpFsSender, FlumeFile> =
    //     &mut state.filesystem.file_tx;
    // let file = FlumeFile {
    //     original_location: None,
    //     final_location: "test.txt".to_string(),
    //     content_stream: Some(rx),
    // };
    //filesystem_sender.append_files(file);
    println!("appended the file");
    drop(state);
    let inner_arc_state = Arc::clone(&arc_state);
    tokio::spawn(async move {
        let mut state = inner_arc_state.write().await;
        println!("past write lock");
        let rx = state.filesystem.proxy_receiver().await;
        println!("got the rx");
        drop(state);
        println!("past dropping state");
        loop {
            if let Ok(bytes) = rx.recv() {
                println!("{:#?}", bytes);
            } else {
                println!("failed to get osme bytes");
            }
        }
        //println!("past loop");
    });
    println!("past the first loop");
    tokio::spawn(async move {
        let mut state = arc_state.write().await;
        state.filesystem.create_state(0, "/".to_string());
        let res = state.filesystem.execute_operation(0).await;
        println!("{:#?}", res);
    });
    println!("past the second");
    while let Some(mut field) = multipart.next_field().await.unwrap() {
        // let file_name = field.file_name().unwrap_or("upload.bin").to_string();
        // let mut file = File::create(format!("/tmp/{file_name}")).await.unwrap();

        // `field` implements Stream<Item = Result<Bytes, MultipartError>>
        while let Some(chunk) = field.chunk().await.unwrap() {
            // file.write_all(&chunk).await.unwrap();
            println!("sent some chunks");
            let _ = tx.send_async(chunk.to_vec()).await;
        }
    }

    StatusCode::OK
}
//SrcAndDest
async fn migrate(
    State(arc_state): State<Arc<RwLock<AppState>>>,
    auth_session: AuthSession,
    headers: HeaderMap,
    Json(request): Json<SrcAndDest>,
) -> impl IntoResponse {
    let mut state = arc_state.write().await;

    let authorized = authorize(&state, auth_session, headers, vec!["manager".to_string()]).await;
    if !authorized {
        return StatusCode::UNAUTHORIZED.into_response();
        //return "unauthorized";
    }

    let migrate_request = MigrateRequest { common: request };
    let _ = migrate_request.node_transport(&mut state).await;

    StatusCode::OK.into_response()
}

// TODO: see if this is really a nessesary route
async fn refresh_status(
    State(arc_state): State<Arc<RwLock<AppState>>>,
    // auth_session: AuthSession,
    // headers: HeaderMap,
) {
    let mut state = arc_state.write().await;
    // let mut authorized = false;
    // if let Some(user) = auth_session.user {
    //     if user.user_perms.iter().any(|user_perm| user_perm.perm == "admin"){
    //         authorized = true;
    //     }
    // }
    // if let Some(token) = get_auth_bearer(headers) {
    //     if resolve_token_perms(state.clone(), token).iter().any(|user_perm| user_perm.perm == "admin"){
    //         authorized = true;
    //     }
    // }
    // if !authorized {
    state.conn_status = {
        if check_channel_health(&state).await {
            Status::Up
        } else {
            Status::Down
        }
    };
    //}
}

async fn fetch_current_node(
    State(arc_state): State<Arc<RwLock<AppState>>>,
    auth_session: AuthSession,
    headers: HeaderMap,
) -> Result<Json<Node>, StatusCode> {
    let state = arc_state.read().await;
    let authorized = authorize(&state, auth_session, headers, vec!["manager".to_string()]).await;
    if !authorized {
        return Err(StatusCode::UNAUTHORIZED);
    }

    if state.current_node.name.is_empty() {
        Err(StatusCode::INTERNAL_SERVER_ERROR)
    } else {
        Ok(Json(Node {
            nodename: state.current_node.name.clone(),
            ip: state.current_node.ip.clone(),
            nodestatus: NodeStatus::Unknown,
            nodetype: state.current_node.nodetype.clone(),
            k8s_type: state.current_node.k8s_type.clone(),
        }))
    }

    // let option_node = state
    //     .database
    //     .retrieve_nodes(state.current_node.name.clone())
    //     .await;
    // if let Some(node) = option_node {
    //     Ok(Json(node))
    // } else {
    //     Err(StatusCode::INTERNAL_SERVER_ERROR)
    // }
}

// TODO: maybe split this function and route into several routes with statuses for diffrent states/nodes/settings?
async fn get_status(
    State(arc_state): State<Arc<RwLock<AppState>>>,
    auth_session: AuthSession,
    headers: HeaderMap,
    Json(request): Json<IncomingMessage>,
) -> impl IntoResponse {
    let state = arc_state.write().await;

    let authorized = authorize(&state, auth_session, headers, vec!["manager".to_string()]).await;
    if !authorized {
        return Err(StatusCode::UNAUTHORIZED);
    }

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
        Ok(Json(returning_req))
    } else if request.message_type == "node" {
        Ok(Json(returning_req))
    } else {
        Ok(Json(returning_req))
    }
}
async fn get_settings(
    State(arc_state): State<Arc<RwLock<AppState>>>,
    auth_session: AuthSession,
    headers: HeaderMap,
) -> impl IntoResponse {
    let state = arc_state.read().await;
    let authorized = authorize(&state, auth_session, headers, vec!["manager".to_string()]).await;
    if !authorized {
        return Err(StatusCode::UNAUTHORIZED);
    }

    match state.database.get_settings().await {
        Ok(settings) => Ok(Json(settings).into_response()),
        Err(_err) => Err(StatusCode::INTERNAL_SERVER_ERROR),
    }
}
async fn get_buttons(
    State(arc_state): State<Arc<RwLock<AppState>>>,
    auth_session: AuthSession,
    headers: HeaderMap,
) -> impl IntoResponse {
    let state = arc_state.read().await;

    let authorized = authorize(&state, auth_session, headers, vec!["manager".to_string()]).await;
    if !authorized {
        return Err(StatusCode::UNAUTHORIZED);
    }

    let mut button_list = vec![];
    match state.database.fetch_all_buttons().await {
        Ok(buttons) => {
            button_list.extend(buttons);
        }
        Err(err) => eprintln!("Error fetching DB buttons: {}", err),
    }
    Ok(Json(List {
        list: ApiCalls::ButtonDataList(button_list),
    }))
}

async fn edit_buttons(
    State(arc_state): State<Arc<RwLock<AppState>>>,
    auth_session: AuthSession,
    headers: HeaderMap,
    Json(request): Json<ModifyElementData>,
) -> impl IntoResponse {
    let state = arc_state.write().await;

    let authorized = authorize(&state, auth_session, headers, vec!["manager".to_string()]).await;
    if !authorized {
        return Err(StatusCode::UNAUTHORIZED);
    }

    let result = state
        .database
        .edit_button_in_db(request)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR);
    result
}
async fn button_reset(
    State(arc_state): State<Arc<RwLock<AppState>>>,
    auth_session: AuthSession,
    headers: HeaderMap,
    Json(request): Json<IncomingMessage>,
) -> impl IntoResponse {
    let state = arc_state.write().await;

    let authorized = authorize(&state, auth_session, headers, vec!["manager".to_string()]).await;
    if !authorized {
        return StatusCode::UNAUTHORIZED;
    }

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
pub struct ConsoleData {
    authcode: String,
    data: String,
    r#type: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct LogLine {
    pub data: String,
}


async fn handle_socket(socket: WebSocket, arc_state: Arc<RwLock<AppState>>) {
    // Acquire lock just to get needed data
    let conn_id = { CONNECTION_COUNTER.fetch_add(1, Ordering::SeqCst) };

    println!("[Conn {}] NEW WEBSOCKET CONNECTION", conn_id);

    let (mut sender, mut receiver) = socket.split();
    //let sender = Arc::new(Mutex::new(sender));

    // let mut broadcast_rx = {
    //     let state = arc_state.read().await;
    //     state.ws_tx.subscribe()
    // };

    // let broadcast_sender = sender.clone();

    let cloned_arc_state = arc_state.clone();

    tokio::spawn(async move {
        let notify = {
            let state = cloned_arc_state.read().await;
            if state.server_console.is_none() {
                Some(state.server_start_event.clone())
            } else {
                None
            }
        }; 

        if let Some(notify) = notify {
            notify.notified().await;
        }
        let state = cloned_arc_state.read().await;
        let server_sender = state.server_console.as_ref().unwrap();
        let mut server_receiver = server_sender.clone().subscribe();
        drop(state);
        while let Ok(msg) = server_receiver.recv().await {
            let _ = sender.send(Message::Text(msg.into())).await;
        };
    });


    // Main receive loop
    while let Some(Ok(message)) = receiver.next().await {
        let locked = {
            let state = arc_state.read().await;
            state.lock
        };
        if !locked {
            if let Message::Text(text) = message {
                let _ = arc_state.write().await.ws_tx.send(text.to_string());
            }
        }
    }

    println!("[Conn {}] DISCONNECTED", conn_id);
}

async fn authorize(
    state: &AppState,
    auth_session: AuthSession,
    headers: HeaderMap,
    perms: Vec<String>,
) -> bool {
    if let Some(user) = auth_session.user {
        if user.user_perms.iter().any(|user_perm| {
            user_perm.perm == "admin"
                || perms
                    .iter()
                    .any(|authorized_perm| *authorized_perm == user_perm.perm)
        }) {
            return true;
        }
    }
    if let Some(token) = get_auth_bearer(headers) {
        if resolve_token_perms(state, token).iter().any(|user_perm| {
            user_perm.perm == "admin"
                || perms
                    .iter()
                    .any(|authorized_perm| *authorized_perm == user_perm.perm)
        }) {
            return true;
        }
    }
    false
}

async fn ws_handler(
    ws: WebSocketUpgrade,
    State(arc_state): State<Arc<RwLock<AppState>>>,
    auth_session: AuthSession,
    headers: HeaderMap,
) -> impl IntoResponse {
    let state = arc_state.write().await;
    let authorized = authorize(&state, auth_session, headers, vec!["manager".to_string()]).await;

    if !authorized {
        return StatusCode::UNAUTHORIZED.into_response();
    }
    drop(state);

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
// and also, this works for SPA and non-SPA content, e.g non-spa for either entire new UI's or
// plugin files which might not be included with the rest of the files
// if its an SPA, there is practically 0 benifit from the server side login middleware
// although for Non-SPA files that served as soley a user convenience feature rather than an actual security
// measure
// also I pass auth layer because sometimes it says that there is no auth layer present instead of serving the webpage
// the fix mainly worked in the router in main, I am just covering future cases
async fn routes_static(
    state: Arc<RwLock<AppState>>,
    _auth_layer: AuthManagerLayer<Backend, MemoryStore>,
) -> (
    Router<Arc<RwLock<AppState>>>,
    Option<OidcAuthLayer<OidcAdditionalClaims>>,
) {
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

    // login_required_middleware will for all files served, will redirect the user back to the main login page
    // if they are not authenticated, for an SPA this does not change anything

    // TODO: remove /oidc? from the end of this
    // need to check to make sure that doesnt break anything
    // during OIDC implimentation i tried several things and have not fully determined the bare configuration
    // needed for OIDC
    let login_url_base = Arc::new(format!("{}", base_path));
    let login_required_middleware = from_fn(
        move |auth_session: AuthSession, req: Request<Body>, next: Next| {
            let login_url = login_url_base.clone();
            async move {
                let path = req.uri().path().to_string();
                if path.starts_with("/oidc") {
                    return next.run(req).await;
                }
                if auth_session.user.is_some() {
                    next.run(req).await
                } else {
                    Redirect::temporary(&login_url).into_response()
                }
            }
        },
    );

    // OIDC layers and routes will not be constructed if there is an issue with creating the layer
    // and routes, and will just merge and empty router
    let mut maybe_oidc_layer: Option<OidcAuthLayer<OidcAdditionalClaims>> = None;
    let mut oidc_routes: Router<Arc<RwLock<AppState>>> = Router::new();

    if let Ok((raw_oidc_layer, _)) = get_oidc_layer().await {
        // adds the callback and the oidc route to actually start the login initiation (includes fallback for /oidc/ if the user adds a path, maybe not nessesary?)
        let callback_router: Router<Arc<RwLock<AppState>>> = Router::new().route(
            "/oidc/callback",
            any(handle_oidc_redirect::<OidcAdditionalClaims>),
        );

        let login_router: Router<Arc<RwLock<AppState>>> = Router::new()
            .route("/oidc", any(oidc_login_initiate))
            .route("/oidc/", any(oidc_login_initiate))
            .layer(
                // Needs a service builder to convert a MiddleWareError into an actual reponse which can be
                // combined with the rest of the routes, this is also in main
                ServiceBuilder::new()
                    .layer(HandleErrorLayer::new(|e: MiddlewareError| async move {
                        eprintln!("OIDC login layer error: {e:#?}");
                        e.into_response()
                    }))
                    .layer(OidcLoginLayer::<OidcAdditionalClaims>::new()),
            );

        oidc_routes = Router::new().merge(callback_router).merge(login_router);

        maybe_oidc_layer = Some(raw_oidc_layer);
    }

    let public = Router::new()
        .route("/", get(handle_static_request))
        .route("/index.html", get(handle_static_request))
        .route("/assets/{*wildcard}", get(handle_static_request));

    let protected = Router::new()
        .route("/{*wildcard}", get(handle_static_request))
        .layer(login_required_middleware);

    let router = Router::new()
        .merge(public)
        .merge(oidc_routes)
        .merge(protected)
        .with_state(state.clone());

    (router, maybe_oidc_layer)
}

// This function will construct a layer with the OIDC client, which includes stuff like the local
// callback URL and importanly the OIDC redirect
// client ids and secrets
async fn get_oidc_layer() -> Result<
    (
        OidcAuthLayer<OidcAdditionalClaims>,
        OidcClient<OidcAdditionalClaims>,
    ),
    Box<dyn Error + Send + Sync>,
> {
    // get_env_var_or_arg("TCPURL", Some(StaticTcpUrl.to_string())).unwrap();

    // TODO: maybe make a function which trims the last / in a url
    let local_url = get_env_var_or_arg("LOCALURL", Some(STATIC_LOCAL_URL.to_string())).unwrap();
    let oidc_callback = if local_url.starts_with("http") {
        format!("{}/oidc/callback", local_url)
    } else {
        format!("http://{}/oidc/callback", local_url)
    };

    let oidc_url: String =
        get_env_var_or_arg("OIDC_URL", Some("http://localhost:5556/dex".into())).unwrap();

    let oidc_secret: String =
        get_env_var_or_arg("OIDC_SECRET", Some("axum-app-secret".into())).unwrap();
    let oidc_id: String = get_env_var_or_arg("OIDC_ID", Some("axum-app".into())).unwrap();

    let client = OidcClient::<OidcAdditionalClaims>::builder()
        .with_default_http_client()
        .with_redirect_url(oidc_callback.parse()?)
        .with_client_id(ClientId::new(oidc_id))
        .with_client_secret(ClientSecret::new(oidc_secret))
        .add_scope(Scope::new("openid".into()))
        .add_scope(Scope::new("profile".into()))
        .discover(IssuerUrl::new(oidc_url.into())?)
        .await?
        .build();

    let layer = OidcAuthLayer::new(client.clone());
    Ok((layer, client))
}

#[axum::debug_handler]
async fn oidc_login_initiate(
    mut auth_session: AuthSession,
    claims: Option<OidcClaims<OidcAdditionalClaims>>,
) -> impl IntoResponse {
    if auth_session.user.is_some() {
        return Redirect::to("/").into_response();
    }
    if let Some(claims) = claims {
        let mut decoded_user = String::new();
        let mut user_perms = Vec::new();

        if let Some(user_perms_claim) = &claims.additional_claims().user_perms {
            user_perms = user_perms_claim.to_vec();
        }

        if let Some(claim_name) = claims.name() {
            if let Some(local_claim_name) = claim_name.get(None) {
                decoded_user = local_claim_name.to_string();
            } else {
                //return StatusCode::INTERNAL_SERVER_ERROR.into_response();
            }
        } else {
            //return StatusCode::INTERNAL_SERVER_ERROR.into_response();
            if let Some(claim_name) = claims.email() {
                decoded_user = claim_name.to_string();
            }
        }

        let user = User {
            username: decoded_user,
            password_hash: None,
            user_perms,
        };
        match auth_session.login(&user).await {
            Ok(_) => eprintln!("login succeeded"),
            Err(e) => {
                eprintln!("login FAILED: {:#?}", e);
                return StatusCode::INTERNAL_SERVER_ERROR.into_response();
            }
        }
        return Redirect::to("/").into_response();
    } else {
        //println!("Invalid claims");
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    }
}

async fn load_settings(
    arc_state: Arc<RwLock<AppState>>,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    let state = arc_state.write().await;

    let settings = match state.database.get_settings().await {
        Ok(s) => s,
        Err(_) => {
            return Err(Box::<dyn Error + Send + Sync>::from(
                "Failed to load settings from database",
            ));
        }
    };
    let _ = state.cached_status_type.send(settings.status_type);

    Ok(())
}

// Because with user perms I am thinking of restricting what settings that can be changed based on user perms
// Instead of submitting a entire new settings feild, you send a list of values you want to change
// it will go through and see if the user can edit those settings
async fn set_settings(
    State(arc_state): State<Arc<RwLock<AppState>>>,
    auth_session: AuthSession,
    headers: HeaderMap,
    Json(request): Json<IncomingMessageWithValue>,
) -> impl IntoResponse {
    let inner_value = request.message;
    let state = arc_state.write().await;

    let authorized = authorize(&state, auth_session, headers, vec![]).await;

    if !authorized {
        return Err(StatusCode::UNAUTHORIZED);
    }

    let settings = match state.database.get_settings().await {
        Ok(s) => s,
        Err(_) => return Err(StatusCode::INTERNAL_SERVER_ERROR),
    };

    let mut settings_value = match serde_json::to_value(settings.clone()) {
        Ok(v) => v,
        Err(_) => return Err(StatusCode::INTERNAL_SERVER_ERROR),
    };

    if let (Value::Object(current), Value::Object(new)) = (&mut settings_value, inner_value) {
        for (k, v) in new {
            current.insert(k, v);
        }
    } else {
        return Err(StatusCode::INTERNAL_SERVER_ERROR);
    }

    let updated_settings: Settings = match serde_json::from_value(settings_value) {
        Ok(s) => s,
        Err(_) => return Err(StatusCode::INTERNAL_SERVER_ERROR),
    };
    let mut has_created = false;
    match state.database.set_settings(updated_settings).await {
        Ok(_) => has_created = true,
        _ => {}
    };
    drop(state);
    let _ = load_settings(arc_state.clone()).await;
    if has_created {
        let _ = notify_node_of_settings(arc_state, Some(settings)).await;
        Ok(StatusCode::CREATED.into_response())
    } else {
        Err(StatusCode::INTERNAL_SERVER_ERROR)
    }
}
async fn notify_node_of_settings(
    arc_state: Arc<RwLock<AppState>>,
    old_settings_option: Option<Settings>,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    let mut state = arc_state.write().await;
    let database = &state.database;
    let settings = database.get_settings().await?;
    if let Some(old_settings) = old_settings_option {
        if !(old_settings.filter == settings.filter) {
            let filter_request = FilterRequest {
                filter: settings.filter,
            };
            let _ = filter_request.node_transport(&mut state).await;
        }
        Ok(())
    } else {
        Ok(())
    }
}
// enum ServerRequests {
//     FilterRequest {
//         filter: Filters
//     },
//     Ping,
//     IntegrationKey {
//         key: String
//     },
//     ServerState {},
//     SrcAndDestFs {},

// }

async fn statistics(
    State(_): State<Arc<RwLock<AppState>>>,
) -> Sse<impl Stream<Item = Result<Event, Box<dyn Error + Send + Sync>>>> {
    let interval = interval(Duration::from_secs(3));

    // let mut authorized = false;
    // if let Some(user) = auth_session.user {
    //     if user.user_perms.iter().any(|user_perm| user_perm.perm == "admin"){
    //         authorized = true;
    //     }
    // }
    // if let Some(token) = get_auth_bearer(headers) {
    //     if !resolve_token_perms(state_clone.clone(), token).iter().any(|user_perm| user_perm.perm == "admin"){
    //         authorized = true;
    //     }
    // }
    // if !authorized {
    //     return Err(StatusCode::UNAUTHORIZED)
    // }

    let system = System::new_all();

    let updates = stream::unfold(
        (interval, system),
        move |(mut interval, mut system)| async move {
            interval.tick().await;
            system.refresh_all();

            let core_data = system.cpus().iter().map(|core| core.cpu_usage()).collect();
            let statistics = Statistics {
                total_memory: system.total_memory(),
                used_memory: system.used_memory(),
                core_data,
                metadata: "".to_string(),
            };
            let event = match serde_json::to_string(&statistics) {
                Ok(json) => Ok(Event::default().data(json)),
                Err(_) => Err("Error".into()),
            };
            Some((event, (interval, system)))
        },
    );
    Sse::new(updates).keep_alive(axum::response::sse::KeepAlive::default())
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
            let status = {
                let state = arc_state.write().await;
                let status_type = state.cached_status_type.borrow().to_string();
                if status_type.is_empty()
                    || status_type == "server-keyword"
                {
                    state.current_node.status.clone()
                } else if status_type == "server-process" {
                    state.poll_server_event.notify_waiters();
                    state.current_node.status.clone()
                } else if status_type == "node" {
                    state.conn_status.clone()
                } else if status_type == "manual-click" {
                    Status::Unknown
                } else {
                    Status::Unknown
                }
            };

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

// TODO: clean up sometime, delete_node should only expect one node type, just verify nothing is using it weirdly
async fn delete_node(
    State(arc_state): State<Arc<RwLock<AppState>>>,
    auth_session: AuthSession,
    headers: HeaderMap,
    Json(request): Json<ModifyElementData>,
) -> impl IntoResponse {
    let state = arc_state.write().await;
    let authorized = authorize(&state, auth_session, headers, vec!["manager".to_string()]).await;
    if !authorized {
        return Err(StatusCode::UNAUTHORIZED);
    }
    drop(state);

    let node_request_name_option = 'node: {
        if let Element::Node(node) = request.element {
            break 'node Some(node.nodename);
        }
        if let Ok(value) = serde_json::to_value(request.element) {
            if let Some(Value::String(nodename)) = value.get("nodename") {
                break 'node Some(nodename.to_string());
            } else {
                break 'node None;
            }
        } else {
            break 'node None;
        }
    };
    if let Some(node_request_name) = node_request_name_option {
        let state = arc_state.write().await;
        let node_option = state.database.retrieve_nodes(node_request_name).await;
        if let Some(node) = node_option {
            if matches!(node.k8s_type, K8sType::None) || matches!(node.k8s_type, K8sType::Unknown) {
                let delete_node_result = state.database.remove_node_in_db_directly(node).await;
                if let Ok(operation_status) = delete_node_result {
                    return Ok(operation_status.into_response());
                } else {
                    return Err(StatusCode::INTERNAL_SERVER_ERROR);
                }
            } else {
                return Err(StatusCode::SERVICE_UNAVAILABLE);
            }
        } else {
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        }
    } else {
        return Err(StatusCode::INTERNAL_SERVER_ERROR);
    }
}
async fn add_node(
    State(arc_state): State<Arc<RwLock<AppState>>>,
    auth_session: AuthSession,
    headers: HeaderMap,
    Json(request): Json<ModifyElementData>,
) -> impl IntoResponse {
    let state = arc_state.write().await;

    let authorized = authorize(&state, auth_session, headers, vec!["manager".to_string()]).await;
    if !authorized {
        return Err(StatusCode::UNAUTHORIZED);
    }

    let result = state
        .database
        .create_nodes_in_db(request)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR);
    result
}
async fn delete_server(
    State(arc_state): State<Arc<RwLock<AppState>>>,
    auth_session: AuthSession,
    headers: HeaderMap,
    Json(request): Json<IncomingMessageWithMetadata>,
) -> impl IntoResponse {
    let mut state = arc_state.write().await;

    let authorized = authorize(&state, auth_session, headers, vec!["manager".to_string()]).await;
    if !authorized {
        return Err(StatusCode::UNAUTHORIZED);
    }

    if let MetadataTypes::DeleteServer {
        delete_server_name,
        delete_server_files: _,
    } = request.metadata.clone()
    {
        let res = state
            .database
            .remove_server_in_db(ModifyElementData {
                element: Element::Server(Server {
                    servername: delete_server_name,
                    ..Default::default()
                }),
                jwt: "".to_string(),
                require_auth: false,
            })
            .await;
        if res.is_err() {
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        }
    }

    let delete_server_request = DeleteServerRequest {
        metadata: request.metadata,
    };
    let _ = delete_server_request.node_transport(&mut state).await;

    Ok(StatusCode::CREATED.into_response())
}

pub async fn start_server(
    State(arc_state): State<Arc<RwLock<AppState>>>,
    auth_session: AuthSession,
    headers: HeaderMap,
) -> impl IntoResponse {
    println!("Called start server");
    let mut state = arc_state.write().await;

    let authorized = authorize(&state, auth_session, headers, vec!["manager".to_string()]).await;
    if !authorized {
        return StatusCode::UNAUTHORIZED.into_response();
    }
    let start_server_request = StartServerRequest {
        stdin: Some(state.ws_tx.subscribe()),
    };
    drop(state);
    if let Ok(mut stream) = start_server_request.stream_transport(arc_state.clone()).await {
        let mut state = arc_state.write().await;
        let server_console: broadcast::Sender<String> = if let Some(console) = state.server_console.as_ref() {
            console.clone()
        } else {
            let (server_console_tx, _) = broadcast::channel::<String>(CHANNEL_BUFFER_SIZE);
            state.server_console = Some(server_console_tx);
            state.server_start_event.notify_waiters();
            state.server_console.as_ref().unwrap().clone()
        };
        drop(state);
         tokio::spawn(async move {
            while let Some(data) = stream.recv().await {
                let _ = server_console.send(serde_json::to_string(&data).unwrap());
            }
         });
    };

    StatusCode::CREATED.into_response()
}

async fn add_server(
    State(arc_state): State<Arc<RwLock<AppState>>>,
    auth_session: AuthSession,
    headers: HeaderMap,
    Json(request): Json<ModifyElementData>,
) -> impl IntoResponse {
    let mut state = arc_state.write().await;
    println!("Got create server request");

    let authorized = authorize(&state, auth_session, headers, vec!["manager".to_string()]).await;
    if !authorized {
        return Err(StatusCode::UNAUTHORIZED);
    }

    let server = match &request.element {
        Element::Server(s) => s.clone(),
        _ => return Ok(StatusCode::BAD_REQUEST),
    };

    if let Ok(settings) = state.database.get_settings().await {
        if settings.disable_custom_servers && server.provider == "custom" {
            return Err(StatusCode::UNAUTHORIZED);
        }
    } else {
        return Err(StatusCode::INTERNAL_SERVER_ERROR);
    }

    state.current_server = Some(server.clone());

    let exists = match state
        .database
        .get_from_servers_database(&server.servername)
        .await
    {
        Ok(result) => result.is_some(),
        Err(e) => {
            println!("{:#?}", e);
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        }
    };

    if exists {
        return Err(StatusCode::CONFLICT);
    }

    if let Err(e) = state.database.create_server_in_db(request).await {
        println!("{:#?}", e);
        return Err(StatusCode::INTERNAL_SERVER_ERROR);
    }

    let settings = state
        .database
        .get_settings()
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let sandbox = {
        if settings.force_sandbox == true {
            true
        } else {
            server.sandbox.clone()
        }
    };
    drop(state);
    let create_server_request = CreateServerRequest {
        metadata: MetadataTypes::Server {
            servername: server.servername.clone(),
            provider: server.provider.clone(),
            providertype: server.providertype.clone(),
            location: server.location.clone(),
            sandbox,
            server_metadata: server.server_metadata.clone(),
        },
    };
    println!("about to initialize the stream");
    if let Ok(mut stream) = create_server_request.stream_transport(arc_state.clone()).await {
        let mut state = arc_state.write().await;
        let server_console: broadcast::Sender<String> = if let Some(console) = state.server_console.as_ref() {
            console.clone()
        } else {
            let (server_console_tx, _) = broadcast::channel::<String>(CHANNEL_BUFFER_SIZE);
            state.server_console = Some(server_console_tx);
            state.server_start_event.notify_waiters();
            state.server_console.as_ref().unwrap().clone()
        };
        drop(state);
         tokio::spawn(async move {
            while let Some(data) = stream.recv().await {
                println!("got message {:#?}", data);
                let _ = server_console.send(serde_json::to_string(&data).unwrap());
            }
         });
    };
    let set_server_request = SetServerRequest {
        metadata: MetadataTypes::Server {
            servername: server.servername.clone(),
            provider: server.provider.clone(),
            providertype: server.providertype.clone(),
            location: server.location.clone(),
            sandbox,
            server_metadata: server.server_metadata.clone(),
        },
    };
    let mut state = arc_state.write().await;
    let _ = set_server_request.node_transport(&mut state).await;

    let server_data_request = ServerDataRequest {
        metadata: MetadataTypes::Server {
            servername: server.servername.clone(),
            provider: server.provider.clone(),
            providertype: server.providertype.clone(),
            location: server.location.clone(),
            sandbox,
            server_metadata: server.server_metadata.clone(),
        },
    };
    let _ = server_data_request.node_transport(&mut state).await;

    Ok(StatusCode::OK)
}

async fn get_integrations(
    State(arc_state): State<Arc<RwLock<AppState>>>,
    auth_session: AuthSession,
    headers: HeaderMap,
) -> impl IntoResponse {
    let state = arc_state.write().await;

    let authorized = authorize(&state, auth_session, headers, vec!["manager".to_string()]).await;
    if !authorized {
        return Err(StatusCode::UNAUTHORIZED);
    }

    let result = state
        .database
        .fetch_all_intergrations()
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR);

    match result {
        Ok(intergrations) => Ok((
            StatusCode::OK,
            Json(List {
                list: ApiCalls::IntergrationsDataList(intergrations),
            }),
        )
            .into_response()),

        Err(status) => Ok(status.into_response()),
    }
}

// Pings the current or specified node
// the status code itself does not determine if a ping was successful rather if the ping
// went through
async fn ping(
    State(arc_state): State<Arc<RwLock<AppState>>>,
    auth_session: AuthSession,
    headers: HeaderMap,
    Json(request): Json<MessagePayload>,
) -> StatusCode {
    let mut state = arc_state.write().await;
    let authorized = authorize(&state, auth_session, headers, vec!["manager".to_string()]).await;
    if !authorized {
        return StatusCode::UNAUTHORIZED;
    }

    if request.message.is_empty() {
        let ping = Ping {};
        let res = ping.node_transport(&mut state).await;
        if res.is_ok() {
            return StatusCode::OK;
        } else {
            return StatusCode::INTERNAL_SERVER_ERROR;
        }
    } else {
        return StatusCode::NOT_IMPLEMENTED;
    }
}

//modify_intergration
async fn modify_intergration(
    State(arc_state): State<Arc<RwLock<AppState>>>,
    auth_session: AuthSession,
    headers: HeaderMap,
    Json(request): Json<ModifyElementData>,
) -> impl IntoResponse {
    let mut state = arc_state.write().await;

    let authorized = authorize(&state, auth_session, headers, vec!["manager".to_string()]).await;
    if !authorized {
        return Err(StatusCode::UNAUTHORIZED);
    }

    if let Element::Intergration(ref intergration_element) = request.element {
        let fetched_intergration_result = state
            .database
            .get_from_intergrations_database(&intergration_element.r#type.to_string())
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR);
        if let Ok(Some(final_intergration)) = fetched_intergration_result {
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
                    .collect();

                for enabled_key in enabled_keys {
                    let hook = settings
                        .iter()
                        .find(|(key, _)| key.starts_with(&format!("_{}_hook", enabled_key)));
                    if let Some(unwrapped_hook) = hook {
                        if let Some(Value::Bool(new_enabled_key)) =
                            intergration_element.settings.get(enabled_key)
                        {
                            if *new_enabled_key == true {
                                let integration_key_request = IntegrationKeyRequest {
                                    key: unwrapped_hook.1.clone(),
                                };
                                let _ = integration_key_request.node_transport(&mut state).await;
                            }
                        }
                    }
                }
            }
        }
    } else {
        return Err(StatusCode::INTERNAL_SERVER_ERROR);
    }

    match state.database.edit_intergrations_in_db(request).await {
        Ok(status_code) => Ok((
            status_code,
            Json(serde_json::json!({
                "success": true,
                "message": "Integration modified successfully"
            })),
        )
            .into_response()),
        Err(e) => {
            let status_code = if let Some(db_err) = e.downcast_ref::<DatabaseError>() {
                resolve_database_error_into_statuscode(db_err.clone())
            } else {
                StatusCode::INTERNAL_SERVER_ERROR
            };

            let error_message = match status_code {
                StatusCode::NOT_FOUND => "Integration not found",
                StatusCode::BAD_REQUEST => "Invalid request data",
                _ => "Internal server error",
            };

            Ok((
                status_code,
                Json(serde_json::json!({
                    "success": false,
                    "error": error_message
                })),
            )
                .into_response())
        }
    }
}

async fn delete_intergration(
    State(arc_state): State<Arc<RwLock<AppState>>>,
    auth_session: AuthSession,
    headers: HeaderMap,
    Json(request): Json<ModifyElementData>,
) -> impl IntoResponse {
    let state = arc_state.write().await;

    let authorized = authorize(&state, auth_session, headers, vec!["manager".to_string()]).await;
    if !authorized {
        return Err(StatusCode::UNAUTHORIZED);
    }

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
    auth_session: AuthSession,
    headers: HeaderMap,
    Json(request): Json<ModifyElementData>,
) -> impl IntoResponse {
    let state = arc_state.write().await;

    let authorized = authorize(&state, auth_session, headers, vec!["manager".to_string()]).await;
    if !authorized {
        return Err(StatusCode::UNAUTHORIZED);
    }

    println!("got request");
    match state.database.create_intergrations_in_db(request).await {
        Ok(status_code) => Ok((
            status_code,
            Json(serde_json::json!({
                "success": true,
                "message": "Integration created successfully"
            })),
        )
            .into_response()),
        Err(e) => {
            let status_code = if let Some(db_err) = e.downcast_ref::<DatabaseError>() {
                resolve_database_error_into_statuscode(db_err.clone())
            } else {
                StatusCode::INTERNAL_SERVER_ERROR
            };

            let error_message = match status_code {
                StatusCode::CONFLICT => "Integration already exists",
                StatusCode::BAD_REQUEST => "Invalid request data",
                _ => "Internal server error",
            };

            Ok((
                status_code,
                Json(serde_json::json!({
                    "success": false,
                    "error": error_message
                })),
            )
                .into_response())
        }
    }
}

// delegate user creation to the DB and return with relevent status code
async fn create_user(
    State(arc_state): State<Arc<RwLock<AppState>>>,
    auth_session: AuthSession,
    headers: HeaderMap,
    Json(request): Json<ModifyElementData>,
) -> impl IntoResponse {
    let state = arc_state.write().await;

    let authorized = authorize(&state, auth_session, headers, vec![]).await;
    if !authorized {
        return Err(StatusCode::UNAUTHORIZED);
    }

    let result = state
        .database
        .create_user_in_db(request)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR);
    result
}
// edits the user data in the db
async fn edit_user(
    State(arc_state): State<Arc<RwLock<AppState>>>,
    auth_session: AuthSession,
    headers: HeaderMap,
    Json(request): Json<ModifyElementData>,
) -> impl IntoResponse {
    let state = arc_state.write().await;
    let authorized = authorize(&state, auth_session, headers, vec![]).await;
    if !authorized {
        return Err(StatusCode::UNAUTHORIZED.into_response());
    }

    let result = state
        .database
        .edit_user_in_db(request)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR);
    Ok(result)
}

// This sets the current server
async fn set_server(
    State(arc_state): State<Arc<RwLock<AppState>>>,
    auth_session: AuthSession,
    headers: HeaderMap,
    Json(request): Json<ModifyElementData>,
) -> Result<StatusCode, StatusCode> {
    let mut state = arc_state.write().await;
    let authorized = authorize(&state, auth_session, headers, vec!["manager".to_string()]).await;
    if !authorized {
        return Err(StatusCode::UNAUTHORIZED);
    }

    if let Element::String(servername) = request.element {
        // its unusual for two ?? but it works
        let retrieved_server = state
            .database
            .get_from_servers_database(&servername)
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
            .transpose()
            .ok_or(StatusCode::NOT_FOUND)??;

        state.current_server = Some(
            Server {
                servername: retrieved_server.servername.clone(),
                provider: retrieved_server.provider.clone(),
                providertype: retrieved_server.providertype.clone(),
                location: retrieved_server.location.clone(),
                node: Node {
                    nodename: state.current_node.name.clone(),
                    ip: state.current_node.ip.clone(),
                    nodestatus: NodeStatus::Unknown,
                    nodetype: state.current_node.nodetype.clone(),
                    k8s_type: state.current_node.k8s_type.clone(),
                }
                .into(),
                // TODO: Have it so if the user has a specific perm, they can create unsandboxed servers
                sandbox: retrieved_server.sandbox.clone(),
                //sandbox: true
                server_metadata: ServerMetadata::default(),
            }
            .into(),
        );

        let set_server_request = SetServerRequest {
            metadata: MetadataTypes::Server {
                servername: retrieved_server.servername,
                provider: retrieved_server.provider,
                providertype: retrieved_server.providertype,
                location: retrieved_server.location,
                sandbox: retrieved_server.sandbox,
                server_metadata: retrieved_server.server_metadata,
            },
        };
        let _ = set_server_request.node_transport(&mut state).await;
        Ok(StatusCode::OK)
    } else {
        Ok(StatusCode::INTERNAL_SERVER_ERROR)
    }
}

// gets the server from the database, if the incoming request is empty, it will give the current server
async fn get_server(
    State(arc_state): State<Arc<RwLock<AppState>>>,
    auth_session: AuthSession,
    headers: HeaderMap,
    Json(request): Json<RetrieveElement>,
) -> Result<Json<Server>, StatusCode> {
    let state = arc_state.write().await;

    let authorized = authorize(&state, auth_session, headers, vec!["manager".to_string()]).await;
    if !authorized {
        return Err(StatusCode::UNAUTHORIZED);
    }

    let mut server_to_get = request.element.clone();
    if state.current_server.is_some() && request.element.is_empty() {
        server_to_get = state.current_server.clone().unwrap().servername;
    }

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
    auth_session: AuthSession,
    headers: HeaderMap,
    Json(request): Json<RetrieveElement>,
) -> impl IntoResponse {
    let state = arc_state.write().await;
    let authorized = authorize(&state, auth_session, headers, vec!["manager".to_string()]).await;
    if !authorized {
        return Err(StatusCode::UNAUTHORIZED.into_response());
    }

    let result = state
        .database
        .get_user_from_database(&request.element)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
        .unwrap();
    Ok(Json(result))
}

// delegate user delection to the DB and returns with relevent status code
async fn delete_user(
    State(arc_state): State<Arc<RwLock<AppState>>>,
    auth_session: AuthSession,
    headers: HeaderMap,
    Json(request): Json<ModifyElementData>,
) -> impl IntoResponse {
    let state = arc_state.write().await;

    let authorized = authorize(&state, auth_session, headers, vec![]).await;
    if !authorized {
        return Err(StatusCode::UNAUTHORIZED.into_response());
    }

    let result = state
        .database
        .remove_user_in_db(request)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR);
    Ok(result)
}

// Capabilities (in this function) notifies the frontend if th backend has certain things enabled, like a
// samba server and etc
async fn capabilities(State(arc_state): State<Arc<RwLock<AppState>>>) -> impl IntoResponse {
    let state = arc_state.write().await;
    let mut capabilities: Vec<String> = vec![];
    capabilities.push("all".to_string());
    Json(capabilities).into_response()
}

// a list of users is returned, like alot of other routes, I need to add permissions, and check against those permissions to see if a user
// can see all the other users, it will delegate the retrival to the database and pass it in as a ApiCalls
async fn users(
    State(arc_state): State<Arc<RwLock<AppState>>>,
    auth_session: AuthSession,
    headers: HeaderMap,
) -> Result<impl IntoResponse, StatusCode> {
    let state = arc_state.write().await;

    let authorized = authorize(&state, auth_session, headers, vec!["manager".to_string()]).await;
    if !authorized {
        return Err(StatusCode::UNAUTHORIZED);
    }

    let users = state
        .database
        .fetch_all()
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(List {
        list: ApiCalls::UserDataList(users),
    }))
}

#[derive(serde::Deserialize)]
struct ChangeNodeRequest {
    node_id: String,
}

#[axum::debug_handler]
async fn change_node(
    State(arc_state): State<Arc<RwLock<AppState>>>,
    auth_session: AuthSession,
    headers: HeaderMap,
    Json(request): Json<ChangeNodeRequest>,
) -> Result<StatusCode, StatusCode> {
    println!("Changing node");
    let state = arc_state.read().await;
    // println!("C");

    let authorized = authorize(&state, auth_session, headers, vec!["manager".to_string()]).await;
    //drop(state);
    if !authorized {
        return Err(StatusCode::UNAUTHORIZED);
    }

    let ws_tx = state.ws_tx.clone();
    let option_node = {
        // let state = state.clone();
        state.database.retrieve_nodes(request.node_id.clone()).await
    };

    drop(state);

    if let Some(node) = option_node {
        {
            arc_state.write().await.cancel_current_conn.cancel();

            // {
            //     let mut state = arc_state.write().await;
            //     state.current_node = NodeWithStream {
            //         name: node.nodename.clone(),
            //         ip: node.ip.clone(),
            //         ..Default::default()
            //     };
            // }

            tokio::spawn(async move {
                let _ = connect_to_server(
                    arc_state.clone(),
                    node.ip.clone(),
                    ws_tx.clone(),
                    false,
                )
                .await;
            });
        }

        Ok(StatusCode::OK)
    } else {
        println!("Error: node not found");
        Err(StatusCode::INTERNAL_SERVER_ERROR)
    }
}

// A list of nodes in a k8s cluster is returned, nothing is returned if there is not a client (k8s support is off)
async fn get_nodes(
    State(arc_state): State<Arc<RwLock<AppState>>>,
    headers: HeaderMap,
    auth_session: AuthSession,
) -> impl IntoResponse {
    let mut state = arc_state.write().await;

    let authorized = authorize(&state, auth_session, headers, vec!["manager".to_string()]).await;
    if !authorized {
        return Err(StatusCode::UNAUTHORIZED.into_response());
    }

    let mut node_list: Vec<NodeWithStream> = vec![];

    if let Clients::K8sLocal(client) = state.client.clone() {
        let request = ListNodeInfoRequest { 
            // connection: client 
        };
        match request.execute_locally(client).await {
            Ok(nodes) => {
                node_list.extend(nodes.clone());
            }
            Err(err) => {
                eprintln!("Error listing nodes: {}", err);
            }
        }
        // match kubernetes::list_node_info(client).await {
        //     Ok(nodes) => {
        //         node_list.extend(nodes.clone());
        //     }
        //     Err(err) => {
        //         eprintln!("Error listing nodes: {}", err);
        //     }
        // }
    }

    match state.database.fetch_all_nodes().await {
        Ok(nodes) => {
            for node in nodes {
                let new_node = NodeWithStream {
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
        let exists = state.additonal_node.iter().any(|n| n.name == node.name);
        if !exists {
            state.additonal_node.push(node);
        }
    }

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

    Ok(Json(List {
        list: ApiCalls::NodeDataList(regular_node_list),
    }))
}

async fn get_servers(
    State(arc_state): State<Arc<RwLock<AppState>>>,
    auth_session: AuthSession,
    headers: HeaderMap,
) -> Result<Json<List>, StatusCode> {
    let state = arc_state.write().await;
    let authorized = authorize(&state, auth_session, headers, vec!["manager".to_string()]).await;
    if !authorized {
        return Err(StatusCode::UNAUTHORIZED);
    }
    let result = match state.database.fetch_all_servers().await {
        Ok(servers) => Ok(Json(List {
            list: ApiCalls::ServerDataList(servers),
        })),
        Err(_) => return Err(StatusCode::INTERNAL_SERVER_ERROR),
    };
    result
}

// Creates a new type for authsession with our custom backend
pub type AuthSession = axum_login::AuthSession<Backend>;

// Claims are important for JWT, particularially for expirery
#[derive(Deserialize, Serialize, Clone)]
pub struct Claims {
    pub exp: usize,
    pub iat: usize,
    pub user: String,
    pub user_perms: Vec<UserPerm>,
}

// Our custom backend, which only hash a list of users
#[derive(Clone)]
pub struct Backend {
    pub users: HashMap<String, User>,
    pub database: Database,
}

impl Backend {
    fn new(database: Database) -> Backend {
        Backend {
            users: HashMap::new(),
            database,
        }
    }
    // fn get_user() -> {
    // }
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
            user_perms: data.claims.user_perms,
        });
        Ok(user)
    }

    async fn get_user(&self, user_id: &String) -> Result<Option<Self::User>, Self::Error> {
        Ok(self.database.retrieve_user(user_id.to_string()).await)
    }
}

// TODO: consider if i need state at all to resolve token perms
fn resolve_token_perms(state: &AppState, token: String) -> Vec<UserPerm> {
    let mut tokens = Vec::new();
    if let Some(env_token) = get_env_var_or_arg::<String>("HEADER_TOKEN", Some(String::new())) {
        tokens.push(env_token.to_string());
    }
    if tokens
        .iter()
        .any(|authorized_token| token == *authorized_token)
    {
        vec![UserPerm {
            perm: "admin".to_string(),
            scope: "all".to_string(),
        }]
    } else {
        vec![]
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

// LoginData arrives as just a user and password
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct LoginData {
    pub user: String,
    pub password: String,
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
async fn sign_out(mut auth_session: AuthSession) -> Result<Response, StatusCode> {
    if let Err(e) = auth_session.logout().await {
        eprintln!("Failed to log in user {e:?}");
        return Ok((StatusCode::INTERNAL_SERVER_ERROR, "Failed to log in").into_response());
    }

    Ok(StatusCode::NO_CONTENT.into_response())
}

#[axum::debug_handler]
async fn sign_in(
    mut auth_session: AuthSession,
    State(arc_state): State<Arc<RwLock<AppState>>>,
    Form(request): Form<LoginData>,
) -> Result<Response, StatusCode> {
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

    let user = User {
        username: user.username,
        password_hash: None,
        user_perms: user.user_perms,
    };

    if let Err(e) = auth_session.login(&user).await {
        eprintln!("Failed to log in user {e:?}");
        return Ok((StatusCode::INTERNAL_SERVER_ERROR, "Failed to log in").into_response());
    }

    Ok((
        StatusCode::OK,
        Json(SignInResponse {
            username: user.username,
        }),
    )
        .into_response())
}

#[derive(serde::Serialize)]
struct UserResponse {
    username: String,
}

#[axum::debug_handler]
async fn user_me(auth_session: AuthSession) -> impl IntoResponse {
    match auth_session.user {
        Some(user) => Json(UserResponse {
            username: user.username,
        })
        .into_response(),
        None => StatusCode::UNAUTHORIZED.into_response(),
    }
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

    let path = if path.exists() {
        path
    } else {
        Path::new("src/frontend/build/index.html").to_path_buf()
    };

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
#[derive(Deserialize, Serialize)]
struct FileChunk {
    file_name: String,
    file_offset: u64,
    file_chunk_size: u64,
}

// This will get the content of a file from gameserver, it will use the custom Tcp filesystem I created
// it will ensure it is not a path escape
#[allow(unused)]
async fn get_files_content(
    State(arc_state): State<Arc<RwLock<AppState>>>,
    auth_session: AuthSession,
    headers: HeaderMap,
    Json(request): Json<FileChunk>,
) -> impl IntoResponse {
    StatusCode::SERVICE_UNAVAILABLE.into_response()
}

// then return the file content
// async fn get_files_content(
//     State(arc_state): State<Arc<RwLock<AppState>>>,
//     auth_session: AuthSession,
//     headers: HeaderMap,
//     Json(request): Json<FileChunk>,
// ) -> Result<impl IntoResponse, impl IntoResponse> {
//     let mut state = arc_state.write().await;
//     let authorized = authorize(&state, auth_session, headers, vec!["manager".to_string()]).await;
//     if !authorized {
//         return Err(StatusCode::UNAUTHORIZED.into_response());
//     }

//     let (tx, rx) = {
//         // let state = arc_state.read().await;
//         state.connection_handler.get_filesystem_stream()
//         //(state.connection_handler.tx.clone(), state.connection_handler.tx.subscribe())
//     };

//     //let mut tcp_fs = TcpFs::new(tx, rx);
//     //let base_path = RemoteFileSystem::new("server", Some(tcp_fs.clone()));

//     let mut base_path = if state.filesystem.is_none() {
//         let mut tcp_fs = TcpFs::new(tx, rx);
//         &mut RemoteFileSystem::new("server", Some(tcp_fs.clone()))
//     } else {
//         state.filesystem.as_mut().unwrap()
//     };
//     //let tcp_fs = base_path.in

//     let user_input = request.file_name.trim_start_matches('/');

//     println!("trying to canonolize");
//     let requested_path = base_path
//         .join(user_input)
//         .canonicalize()
//         .await
//         .map_err(|e| {
//             eprintln!("Invalid path: {}", e);
//             (StatusCode::BAD_REQUEST, "Invalid path").into_response()
//         })?;

//     println!("going to get the path structure");
//     let (dir_path, file_name) = match (requested_path.parent(), requested_path.file_name()) {
//         (Some(dir), Some(file)) => (dir.to_path_buf(), file.to_os_string()),
//         _ => {
//             eprintln!("Could not split path into directory and file");
//             return Err((StatusCode::BAD_REQUEST, "Invalid path structure").into_response());
//         }
//     };

//     println!("resolving path");
//     let full_path = dir_path.join(&file_name);
//     let file_chunk = FileChunk {
//         file_name: full_path.to_string_lossy().to_string(),
//         file_chunk_offet: request.file_chunk_offet,
//         file_chunk_size: request.file_chunk_size,
//     };

//     let content = base_path
//         .get_files_content(file_chunk)
//         .await
//         .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, "Failed to read file").into_response())?;

//     println!("doing something with file content");
//     Ok(Json(content))
// }

// Gets a list of files and return to to things like the filebrowser
#[allow(unused)]
pub async fn get_files(
    State(arc_state): State<Arc<RwLock<AppState>>>,
    headers: HeaderMap,
    auth_session: AuthSession,
    Json(request): Json<IncomingMessage>,
) -> impl IntoResponse {
    let state = arc_state.write().await;
    let request = LsRequest {
        id: 0,
        location: "/home/projects/gameserver-rs/gameserver".to_string(),

    };
    // match request.node_transport(&state).await {
    //     Ok(()) => {
    //         println!("successfully sent it");
    //     }
    //     Err(_) => todo!(),
    // }

    StatusCode::SERVICE_UNAVAILABLE.into_response()
    // Json(List {
    //     list: ApiCalls::FileDataList(items),
    // })
    // .into_response()
}

fn get_auth_bearer(headers: HeaderMap) -> Option<String> {
    headers
        .get("Authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .map(|token| token.to_string())
}


// pub async fn stream_file_download(
//     State(arc_state): State<Arc<RwLock<AppState>>>,
//     auth_session: AuthSession,
//     headers: HeaderMap,
//     axum::extract::Path(file_path): axum::extract::Path<String>,
// ) -> Result<Response<Body>, StatusCode> {
//     let state = arc_state.read().await;
//     let authorized = authorize(&state, auth_session, headers, vec!["manager".to_string()]).await;
//     if !authorized {
//         return Err(StatusCode::UNAUTHORIZED);
//     }
//     drop(state);

//     let tcp_fs = {
//         let state = arc_state.read().await;
//         let (tx, rx) = state.connection_handler.get_filesystem_stream();
//         //(state.connection_handler.proxy_tx.clone(), state.connection_handler.proxy_rx.resubscribe());
//         Arc::new(Mutex::new(TcpFs::new(tx, rx)))
//     };

//     let decoded_path = urlencoding::decode(&file_path)
//         .map_err(|_| StatusCode::BAD_REQUEST)?
//         .to_string();

//     let normalized_path = normalize_and_secure_path(&decoded_path)?;

//     let metadata = {
//         let fs = tcp_fs.lock().await;
//         let mut remote_fs = RemoteFileSystem::new(&normalized_path, Some((*fs).clone()));

//         let is_file = remote_fs.is_file().await.map_err(|e| {
//             eprintln!("Error checking if path is file: {}", e);
//             StatusCode::NOT_FOUND
//         })?;

//         if !is_file {
//             return Err(StatusCode::BAD_REQUEST);
//         }

//         remote_fs.ensure_metadata().await.map_err(|e| {
//             eprintln!("Error getting metadata: {}", e);
//             StatusCode::INTERNAL_SERVER_ERROR
//         })?;

//         remote_fs.cached_metadata.clone()
//     };

//     let file_size = metadata.as_ref().and_then(|m| m.file_size);
//     let chunk_size = 64 * 1024;

//     let stream = TcpFileStream::new(
//         tcp_fs.clone(),
//         normalized_path.clone(),
//         file_size,
//         chunk_size,
//     );

//     let body = Body::from_stream(stream);

//     let filename = std::path::Path::new(&normalized_path)
//         .file_name()
//         .and_then(|n| n.to_str())
//         .unwrap_or("download");

//     let mut response = Response::new(body);
//     let headers = response.headers_mut();

//     headers.insert(
//         header::CONTENT_TYPE,
//         "application/octet-stream".parse().unwrap(),
//     );
//     headers.insert(
//         header::CONTENT_DISPOSITION,
//         format!("attachment; filename=\"{}\"", filename)
//             .parse()
//             .unwrap(),
//     );

//     if let Some(size) = file_size {
//         headers.insert(header::CONTENT_LENGTH, size.to_string().parse().unwrap());
//     } else {
//         headers.insert(header::TRANSFER_ENCODING, "chunked".parse().unwrap());
//     }

//     Ok(response)
// }
// fn normalize_and_secure_path(path: &str) -> Result<String, StatusCode> {
//     use std::path::{Component, PathBuf};

//     let path = path.trim();

//     let path_buf = PathBuf::from(path);

//     let mut normalized = PathBuf::new();
//     for component in path_buf.components() {
//         match component {
//             Component::Normal(part) => normalized.push(part),
//             Component::RootDir => {
//                 continue;
//             }
//             Component::CurDir => {
//                 continue;
//             }
//             Component::ParentDir => {
//                 if normalized.components().count() > 0 {
//                     normalized.pop();
//                 }
//             }
//             _ => {
//                 return Err(StatusCode::BAD_REQUEST);
//             }
//         }
//     }

//     let normalized_str = normalized.to_string_lossy().to_string();

//     let server_prefix = "server/";
//     let final_path = if normalized_str.starts_with(server_prefix) {
//         normalized_str
//     } else if normalized_str.is_empty() {
//         return Err(StatusCode::BAD_REQUEST);
//     } else {
//         format!("{}{}", server_prefix, normalized_str)
//     };

//     if !final_path.starts_with(server_prefix) {
//         eprintln!("Path traversal attempt blocked: {} -> {}", path, final_path);
//         return Err(StatusCode::FORBIDDEN);
//     }

//     if final_path.contains('\0') {
//         eprintln!("Null byte in path blocked: {}", final_path);
//         return Err(StatusCode::BAD_REQUEST);
//     }

//     Ok(final_path)
// }

// Unit tests
#[cfg(test)]
mod tests {
    //use std::any::Any;

    use super::*;

    #[allow(unused)]
    async fn create_app_state_for_tests() -> Result<AppState, Box<dyn std::error::Error + Send + Sync>> {

        let conn = first_connection().await?;
        let database = database::Database::new(Some(conn));

        let database_conn_result = database.ensure_database_conn().await;

        ensure_admin_user(database.clone()).await;

        if let Err(err) = database_conn_result {
            println!("{}", err);
        }

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


        // Overrides for testing or specific cases where how it works a setup may be diffrent
        let enable_k8s_client: bool = get_env_var_or_arg("ENABLE_K8S_CLIENT", Some(true)).unwrap();

        // creates a websocket broadcase and tcp channels
        let (ws_tx, _) = broadcast::channel::<String>(CHANNEL_BUFFER_SIZE);

        // sets the client to be none by default unless this is ran the stanard way which will be ran with the appropriate feature-flag
        // which will set the k8s client
        let mut client: Clients = Clients::None;
        if enable_k8s_client && K8S_WORKS {
            client = Clients::K8sLocal(
                K8sClient { 
                    k8s_client: Client::try_default().await?, 
                    docker_info: String::new()
                }
            );
        }

        // let mut node_url: String = config_node_url.to_string();
        // if !dont_override_conn_with_k8s && let Clients::K8s(ref inner_client) = client {
        //     if let Ok(url_result) = &kubernetes::get_avalible_gameserver(&inner_client).await {
        //         node_url = url_result.clone();
        //     } else {
        //         println!(
        //             "Could not get a successful url for a existing gameserver, will try the fallback url"
        //         )
        //     }
        // }

        let mut nodes: Vec<NodeWithStream> = vec![];
        if let Ok(db_nodes) = database.fetch_all_nodes().await {
            nodes = db_nodes
                .into_iter()
                .map(|node| NodeWithStream {
                    name: node.nodename,
                    nodetype: node.nodetype,
                    ip: node.ip,
                    ..Default::default()
                })
                .collect()
        }

        let (internal_tx, internal_rx) = broadcast::channel::<Vec<u8>>(100);

        let mut rcon_connection: Option<Arc<Mutex<Connection<TcpStream>>>> = None;
        if let Ok(retrived_db) = database.get_settings().await {
            if retrived_db.enabled_rcon {
                rcon_connection = match Connection::builder()
                    .enable_minecraft_quirks(true)
                    .connect(&retrived_db.rcon_url, &retrived_db.rcon_password)
                    .await
                {
                    Ok(conn) => Some(Arc::new(Mutex::new(conn))),
                    Err(e) => {
                        eprintln!("Failed to connect to RCON: {}", e);
                        None
                    }
                }
            }
        }

        let current_server = None;
        // if !(database.get_settings().await?.current_server.into_server() == Server::default()) {
        //     current_server = Some(database.get_settings().await?.current_server.into_server())
        // }

        let connection_handler = ConnectionHandler::new();

        let (fs_sender_tx, fs_sender_rx) = flume::unbounded();
        let (fs_receiver_tx, fs_receiver_rx) = flume::unbounded();
        let filesystem = FileSystemHandler::new(fs_sender_tx, fs_sender_rx, fs_receiver_tx, fs_receiver_rx);
        // filesystem.set_start_delimiter("\\f".as_bytes().to_vec());
        // filesystem.set_end_delimiter("//f".as_bytes().to_vec());

        let cached_status_type = watch::channel(String::new()).0;

        let state: AppState = AppState {
            // tx: tx,
            // rx: rx,
            connection_handler,
            cancel_current_conn: CancellationToken::new(),
            internal_rx: Some(internal_rx.resubscribe()),
            internal_tx: Some(internal_tx),
            ws_tx: ws_tx.clone(),
            // ws_rx: ws_rx.resubscribe(),
            server_console: None,
            server_start_event: Arc::new(Notify::new()),
            base_path: base_path.clone(),
            current_node: NodeWithStream::default(),
            database: database.clone(),
            client,
            additonal_node: nodes,
            conn_status: Status::Unknown,
            cached_status_type,
            poll_server_event: Arc::new(Notify::new()),
            rcon_connection,
            current_server,
            lock: false,
            filesystem,
        };
        Ok(state)
    }

    /*
    shared tests (which should work on both bare-metal and k8s)
    */
    mod shared {
        use super::*;
        use crate::database::{Database, Element};

        async fn create_db_for_tests() -> Result<Database, Box<dyn std::error::Error + Send + Sync>>
        {
            let conn = first_connection().await?;
            let database: Database = database::Database::fix_connection(Some(conn)).await;
            database
                .ensure_database_conn()
                .await
                .expect("Failed to ensure db structure");
            Ok(database)
        }

        mod users {
            use serial_test::serial;

            use super::*;

            #[tokio::test]
            #[serial]
            async fn remove_user() {
                let database: Database = create_db_for_tests().await.unwrap();
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

                let _ = database.create_user_in_db(user).await.expect("Failed");

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

                assert!(remove_user_result.is_ok());
            }

            #[tokio::test]
            #[serial]
            async fn create_user_perms() {
                let database: Database = create_db_for_tests().await.unwrap();
                database.clear_db().await.expect("Failed to clear DB");

                let user = ModifyElementData {
                    element: Element::User {
                        user: "kk".to_owned(),
                        password: "ddd".to_owned(),
                        user_perms: vec![UserPerm {
                            perm: "test".to_string(),
                            scope: "all".to_string(),
                        }],
                    },
                    require_auth: true,
                    jwt: "".to_owned(),
                };

                let _ = database.create_user_in_db(user).await;

                let retrieved_user_option = database.retrieve_user("kk".to_string()).await;

                if let Some(retrieved_user) = retrieved_user_option {
                    assert_eq!(
                        retrieved_user.user_perms,
                        vec![UserPerm {
                            perm: "test".to_string(),
                            scope: "all".to_string()
                        }]
                    );
                } else {
                    panic!();
                }
            }

            #[tokio::test]
            #[serial]
            async fn create_user() {
                let database: Database = create_db_for_tests().await.unwrap();
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
                assert!(create_user_result.is_ok());
            }

            #[tokio::test]
            #[serial]
            async fn duplicate_user() {
                let database: Database = create_db_for_tests().await.unwrap();
                database.clear_db().await.expect("Failed to clear DB");

                let user_a = ModifyElementData {
                    element: Element::User {
                        user: "A".to_owned(),
                        password: "test".to_owned(),
                        user_perms: vec![],
                    },
                    require_auth: true,
                    jwt: "".to_owned(),
                };

                let user_b = ModifyElementData {
                    element: Element::User {
                        user: "A".to_owned(),
                        password: "test".to_owned(),
                        user_perms: vec![],
                    },
                    require_auth: true,
                    jwt: "".to_owned(),
                };

                let _ = database.create_user_in_db(user_a).await;
                let result_b = database.create_user_in_db(user_b).await;

                assert!(result_b.is_err());
            }
        }

        mod nodes {
            use serial_test::serial;

            use super::*;

            #[tokio::test]
            #[serial]
            async fn create_node() {
                let database: Database = create_db_for_tests().await.unwrap();
                database.clear_db().await.expect("Failed to clear DB");

                let node = ModifyElementData {
                    element: Element::Node(Node {
                        nodename: "main".to_string(),
                        ip: STATIC_LOCAL_URL.to_string(),
                        nodestatus: NodeStatus::Unknown,
                        nodetype: NodeType::Custom(None),
                        k8s_type: K8sType::Unknown,
                    }),
                    jwt: "".to_string(),
                    require_auth: true,
                };

                let result = database.create_nodes_in_db(node).await;
                assert!(result.is_ok());
            }
        }
    }

    /*
    bare-metal specific tests
    */
    #[cfg(not(any(feature = "full-stack", feature = "docker", feature = "database")))]
    mod internal {}

    /*
    k8s and sqlx tasks
    */
    #[cfg(any(feature = "full-stack"))]
    mod k8s {
        use super::*;
        use crate::database::Database;

        async fn create_db_for_tests() -> Result<Database, Box<dyn std::error::Error + Send + Sync>>
        {
            let conn = first_connection().await?;
            let database: Database = database::Database::fix_connection(Some(conn)).await;
            database
                .ensure_database_conn()
                .await
                .expect("Failed to ensure db structure");
            Ok(database)
        }

        // #[tokio::test]
        // #[serial]
        // async fn has_k8s_client(){
        //     Client::try_default().await?;
        //     assert!(true);
        // }

        mod server {
            use serial_test::serial;

            use super::*;

            #[tokio::test]
            #[serial]
            async fn create_server() {
                let database: Database = create_db_for_tests().await.unwrap();
                database.clear_db().await.expect("Failed to clear DB");

                let server = ModifyElementData {
                    element: database::databasespec::Element::Server(Server {
                        servername: "test".to_string(),
                        provider: "".to_string(),
                        providertype: "".to_string(),
                        location: "test".to_string(),
                        sandbox: false,
                        node: Node {
                            nodename: "test".to_string(),
                            ip: "127.0.0.1:8080".to_string(),
                            nodestatus: NodeStatus::Unknown,
                            nodetype: NodeType::Custom(None),
                            k8s_type: K8sType::Unknown,
                        },
                        server_metadata: ServerMetadata::default(),
                    }),
                    jwt: "".to_string(),
                    require_auth: false,
                };

                let result = database.create_server_in_db(server).await;
                //assert!(result.is_ok());
                if result.is_err() {
                    assert!(false);
                    //panic!("{:#?}", result);
                } else {
                    assert!(true);
                }
            }

            #[tokio::test]
            #[serial]
            async fn create_server_without_name() {
                let database: Database = create_db_for_tests().await.unwrap();
                database.clear_db().await.expect("Failed to clear DB");

                let server = ModifyElementData {
                    element: database::databasespec::Element::Server(Server {
                        servername: "".to_string(),
                        provider: "".to_string(),
                        providertype: "".to_string(),
                        location: "test".to_string(),
                        sandbox: false,
                        node: Node {
                            nodename: "test".to_string(),
                            ip: "127.0.0.1:8080".to_string(),
                            nodestatus: NodeStatus::Unknown,
                            nodetype: NodeType::Custom(None),
                            k8s_type: K8sType::Unknown,
                        },
                        server_metadata: ServerMetadata::default(),
                    }),
                    jwt: "".to_string(),
                    require_auth: false,
                };

                let result = database.create_server_in_db(server).await;
                assert!(result.is_err());
            }
        }

        mod node_connection {
            use serial_test::serial;

            use super::*;

            #[tokio::test]
            #[serial]
            async fn try_initial_connection_test() {
                let (ws_tx, _) = broadcast::channel::<String>(CHANNEL_BUFFER_SIZE);
                let (tx, _) = broadcast::channel::<Vec<u8>>(CHANNEL_BUFFER_SIZE);

                let node_url = get_env_var_or_arg("TCPURL", Some(STATIC_NODE_URL.to_string())).unwrap();

                let initial_connection_attempts: u64 =
                    get_env_var_or_arg("INITIAL_CONNECTION_ATTEMPTS", Some(5)).unwrap();

                let initial_connection_timeout: u64 =
                    get_env_var_or_arg("INITIAL_CONNECTION_TIMEOUT", Some(2)).unwrap();

                let state = Arc::new(RwLock::new(create_app_state_for_tests().await.unwrap()));

                let result = try_initial_connection(
                    initial_connection_attempts,
                    initial_connection_timeout,
                    false,
                    &state,
                    node_url,
                    &ws_tx,
                    tx,
                )
                .await;

                assert!(result.is_ok());
            }
        }
    }

    mod http {}
}
