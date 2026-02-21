use file_transfer_system::server;
use hostname::get;
use serde_json::{json, Value};
use std::convert::TryFrom;
use std::io::Error;
use std::ffi::OsString;
use std::fmt;
use std::io::{ErrorKind, SeekFrom};
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::path::Path;
use std::collections::HashMap;
use tokio::fs;
use tokio::fs::OpenOptions;
use tokio::io;
use tokio::io::{split, AsyncBufReadExt, AsyncReadExt, AsyncSeekExt, AsyncWriteExt, BufReader};
use tokio::net::TcpListener;
use tokio::process::Child;
use tokio::process::{ChildStdin, Command as TokioCommand};
use tokio::sync::{mpsc, Mutex};

use crate::broadcast::Sender;
use crate::filesystem::cleanup_end_file_markers;
use crate::filesystem::get_files_content;
use crate::filesystem::get_metadata;
use crate::filesystem::list_directory;
use crate::filesystem::list_directory_with_range;
use crate::filesystem::send_folder_over_broadcast;
use crate::filesystem::BasicPath;
use crate::filesystem::FileChunk;
use crate::filesystem::FileOperations;
use crate::filesystem::execute_file_operation;
use crate::FileOperations::FileMoveOperation;
use crate::FileOperations::FileDownloadOperation;
use crate::FileOperations::FileZipOperation;
use crate::FileOperations::FileUnzipOperation;
use crate::FileOperations::FileCopyOperation;

use crate::providers::Custom;
use crate::providers::Minecraft;
use crate::providers::Provider;
use crate::providers::ProviderConfig;
use crate::providers::ProviderType;

use tokio::fs::File;
use tokio::io::AsyncRead;
use tokio::net::TcpStream;
use uuid::Uuid;

use std::net::SocketAddr;
use tokio::sync::broadcast;

//use futures::TryFutureExt;

// I use the same code as in the main server
// with a few diffrences in stuff like filesystem
mod extra;
mod filesystem;
mod intergrations;
mod providers;
mod jsondatabase;
mod databasespec;

use databasespec::ServerIndex;

use jsondatabase::{load_db, save_db};

use intergrations::{run_intergration_commands, IntergrationCommands};

// const ENABLE_BROADCAST_LOGS: bool = true;

// Server directory as in the one at the root of this project (../server)
// all server files are sandboxed in there including nested server directories
// by default its set to well, server, and changing this means that it will look for a diffrent directory at the root
// for server files
const SERVER_DIR: &str = "server";

// Old, at some point move to IncomingMessageWithMetadata
// a struct for basic message sending between a node and the main server
// IncomingMessageWithMetadata and IncomingMessage should be renamed to something that makes sense
#[derive(Debug, serde::Serialize, serde::Deserialize, Clone)]
struct IncomingMessage {
    message: String,
    #[serde(rename = "type")]
    message_type: String,
    authcode: String,
}

// newer version of IncomingMessage, mainly because this includes the metadata feild which i sometimes use
// IncomingMessageWithMetadata and IncomingMessage should be renamed to something that makes sense
// Note, this also handles the things like MessagePayloadWithMetadata and converts it here, as 
// from the gameservers perpective, the command payload is incoming, so it made sense not to recreate such a struct
#[derive(Debug, serde::Serialize, serde::Deserialize)]
struct IncomingMessageWithMetadata {
    message: String,
    #[serde(rename = "type")]
    message_type: String,
    metadata: MetadataTypes,
    authcode: String,
}

// Metadata types, currently i primarially use it to transmit server data
// but it can be not set or set as a string too, there will probably be more metadata types in the future
#[derive(Debug, serde::Serialize, serde::Deserialize, Clone)]
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

// a struct primarially used for node migration, as in, moving the server files
// but will probably used to all sorts of transfers in the future
#[derive(Debug, serde::Serialize, serde::Deserialize, Clone)]
struct SrcAndDest {
    src: ApiCalls,
    dest: ApiCalls,
    metadata: String,
}

// NodeStatus
// as in, if servers can be manually or automatically sceduled to it
// which depends if its avalible, or several other factors which will affect how the node can scedule
// servers, immutable varients represent kubernetes nodes, which cant just be removed as of now,
// because it doesnt seem to make much sense to hide it in a cluster
#[derive(Debug, serde::Serialize, serde::Deserialize, Clone, PartialEq)]
#[serde(tag = "kind", content = "data")]
pub enum NodeStatus {
    Enabled,
    Disabled,
    ImmutablyEnabled,
    ImmutablyDisabled,
}

// NodeTypes, this might be unnessesary, but for now its useful to represent nodes like the one the
// server will try connecting to initially, and key nodes which the user doesnt define but is picked up
// for better usability, custom is what the user creates manually and at some point, it might be added where the
// user can disable their custom ones or detected ones
#[derive(Debug, serde::Serialize, serde::Deserialize, Clone, PartialEq)]
#[serde(tag = "kind", content = "data")]
pub enum NodeType {
    Custom,
    Main,
}

// A simple node, the only reason this is in this node is mainly for server migrations, nodename and ip is the feilds currently
// used but i keep the other ones for consistency
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct Node {
    pub nodename: String,
    pub ip: String,
    pub nodetype: NodeType,
    pub nodestatus: NodeStatus,
}

// A list will contain stuff like a list of files, resources, capabilities, or things of that nature
#[derive(Debug, serde::Serialize, serde::Deserialize)]
struct List {
    list: Vec<String>,
}

// This is even older than IncomingMessage, and should be phased out soon
#[derive(Debug, serde::Serialize, serde::Deserialize, Clone)]
struct MessagePayload {
    r#type: String,
    message: String,
    authcode: String,
}

// ApiCalls represent some common types so I can keep track of them, its not used them much
// and might be worth phasing out in the future, its definitately used in the main server for mixed data types and sending
// them over a common interface
#[derive(Debug, serde::Deserialize, serde::Serialize, Clone)]
#[serde(tag = "kind", content = "data")]
enum ApiCalls {
    None,
    Capabilities(Vec<String>),
    NodeList(Vec<String>),
    IncomingMessage(MessagePayload),
    Node(Node),
    FileDownloadOperation(String),
    FileMoveOperation(String),
    FileZipOperation(String),
    FileUnzipOperation(String),
    FileCopyOperation(String)
}

impl From<ApiCalls> for FileOperations {
    fn from(api_call: ApiCalls) -> Self {
        match api_call {
            ApiCalls::FileDownloadOperation(s) => FileOperations::FileDownloadOperation(s),
            ApiCalls::FileMoveOperation(s) => FileOperations::FileMoveOperation(s),
            ApiCalls::FileZipOperation(s) => FileOperations::FileZipOperation(s),
            ApiCalls::FileUnzipOperation(s) => FileOperations::FileUnzipOperation(s),
            ApiCalls::FileCopyOperation(s) => FileOperations::FileCopyOperation(s),
            _ => FileOperations::Unknown
        }
    }
}

// I tried to convert from a Value, as in undefined data type, to a List, as its a data type created only
// here and its used sometimes, maybe it would be better to just do the conversion when its needed
impl TryFrom<Value> for List {
    type Error = &'static str;

    fn try_from(value: Value) -> Result<Self, Self::Error> {
        if let Some(full_struct) = value.get("list") {
            if let Some(Value::Array(list)) = full_struct.get("data") {
                return Ok(List {
                    list: list
                        .iter()
                        .filter_map(|v| v.as_str().map(|s| s.to_string()))
                        .collect(),
                });
            }
        }

        Err("Value does not represent a NodeList variant")
    }
}

// These ip:port defaults are diffrence based on feature as i typically do not run full-stack
// when im testing on bare metal, where the ip:port have to be diffrent to not conflict
#[cfg(feature = "full-stack")]
static StaticLocalUrl: &str = "0.0.0.0:8080";

#[cfg(not(feature = "full-stack"))]
static StaticLocalUrl: &str = "0.0.0.0:8082";

// #[cfg(feature = "full-stack")]
// static PORT: &str = "8080";

// #[cfg(not(feature = "full-stack"))]
// static PORT: &str = "8082";

// the server state, currently only holds keywords for what messages to look for when declaring the server as started or stopped
// might be phased out in favor of determining whether or not the process is running or not
#[derive(serde::Serialize)]
struct GetState {
    start_keyword: String,
    stop_keyword: String,
}

// runs a command and forwards the output of the command to the given channel, which in this case would be back to
// the main server
async fn run_command_live_output(
    state: &AppState,
    cmd: Command,
    label: String,
    sender: Option<mpsc::Sender<String>>,
    stdin_arc: Option<Arc<Mutex<Option<ChildStdin>>>>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let mut tokio_cmd = TokioCommand::from(cmd);
    tokio_cmd
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .stdin(Stdio::piped());
    let mut child = tokio_cmd.spawn()?;

    if let Some(stdin_slot) = stdin_arc {
        let child_stdin = child.stdin.take();
        *stdin_slot.lock().await = child_stdin;
    }

    if let Some(stdout) = child.stdout.take() {
        let tx = sender.clone();
        let lbl = label.clone();
        tokio::spawn(async move {
            let mut reader = BufReader::new(stdout).lines();
            while let Ok(Some(line)) = reader.next_line().await {
                if let Some(tx) = &tx {
                    let msg =
                        json!({"type":"stdout","data":format!("[{}] {}", lbl, line)}).to_string();
                    let _ = tx.send(msg).await;
                }
            }
        });
    }

    if let Some(stderr) = child.stderr.take() {
        let tx = sender.clone();
        let lbl = label.clone();
        tokio::spawn(async move {
            let mut reader = BufReader::new(stderr).lines();
            while let Ok(Some(line)) = reader.next_line().await {
                if let Some(tx) = &tx {
                    let msg =
                        json!({"type":"stderr","data":format!("[{}] {}", lbl, line)}).to_string();
                    let _ = tx.send(msg).await;
                }
            }
        });
    }

    let status = child.wait().await?;
    Ok(())
}

// Custom metadata for file, not its actual metadata, as it might not be relevent for my file tree
// sandboxing, and determining all of a folders children was listed
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct FsMetadata {
    pub is_file: bool,
    pub is_dir: bool,
    pub optional_folder_children: Option<u64>,
    pub canonical_path: String,
}

// this is FsMetadata but simpler, should be phased out in favor of FsMetadata
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct FsEntry {
    pub name: String,
    pub is_file: bool,
    pub is_dir: bool,
}

// Due to certain instabillity when it comes to sending files and file content, id matching is required to
// make sure the correct data is matched to the correct file or operation
#[derive(serde::Serialize, serde::Deserialize, Debug)]
struct FileRequestMessage {
    id: u64,
    #[serde(flatten)]
    payload: FileRequestPayload,
}

// The types of file requests the server can make, easy to match and keep track of/consistent
#[derive(serde::Serialize, serde::Deserialize, Debug)]
#[serde(tag = "type", content = "data")]
enum FileRequestPayload {
    Metadata {
        path: String,
    },
    ListDir {
        path: String,
    },
    ListDirWithRange {
        path: String,
        start: Option<u64>,
        end: Option<u64>,
    },
    PathFromTag {
        path: String,
        tag: Option<String>,
    },
    FileChunk(FileChunk),
}

// Needs to be phased out, or just removed, everything now uses FileRequestPayload
#[derive(serde::Serialize, serde::Deserialize)]
struct FileResponseMessage {
    in_response_to: u64,
    data: serde_json::Value,
}

// There are two main read modes, Json for message processing and MigrationFile for file transfers, althought it should
// be renamed to just file transfer, as it switches to this mode during a migration or from the file transfers from the server
// the reason I seperated it is because of the fact that raw bytes are sent very quickly from a node or the main server
// which can cause some weirdness if mixed with the json logic, and seperations of concerns
// TODO: change the name from MigrationFile to something more represenative of the use cases
enum ReadMode {
    Json,
    MigrationFile {
        current_file: tokio::fs::File,
        file_name: String,
        bytes_written: u64,
    },
    // NormalFile {
    //     current_file: tokio::fs::File,
    // },
}


// AppState for the Node, stores the name of the current server, the state of the process
// whether or not its running, the channel for the output messages, and the process, makes it easier to pass and modify
// between functions
#[derive(Clone)]
struct AppState {
    current_server: Option<String>,
    //server_index: HashMap<String, ServerIndex>,
    server_running: Arc<Mutex<bool>>,
    server_output_tx: Arc<Mutex<Option<broadcast::Sender<String>>>>,
    server_process: Arc<Mutex<Option<Child>>>,
    // I was wondering whether or not to use an Arc Mutex for the DB, in my main server I both have the db behind a arc mutex (behind app state)
    // and both write queries upon any updates to the db, I have to have both, as I need to ensure when im writing new data to the database, there isnt an operation going on
    // based on an earlier state of the database which will write it after such an operation leading to some of the newer data getting deleted
    db: Arc<Mutex<jsondatabase::Database>>,
}

// Will remove this, this was kept because at a time there was a issue with the channels reciving messages they sent, so
// i made it ignore messages still in the wrapped, but this is no longer needed
// TODO: remove this
#[derive(serde::Serialize, serde::Deserialize)]
struct OneTimeWrapper {
    data: Value,
}

// This is for returning a connection from either a specifed ip feild, which might look like
// <IP>:<PORT> or IP and PORT seprately from two diffrent arguments, I need to probably enforce setting the ip or port, atleast change to the default port
// but for now it suffices
// TODO: do above
// TODO: remove the option for ip (as its required anyways)
pub async fn unsure_ip_or_port_tcp_conn(
    ip: Option<String>,
    port: Option<String>,
) -> Result<TcpStream, Box<dyn std::error::Error + Send + Sync>> {
    let ip = ip.ok_or("IP is required")?;
    let (host, extracted_port) = if let Some(idx) = ip.rfind(':') {
        let (host_part, port_part) = ip.split_at(idx);
        let port_part = &port_part[1..];
        (host_part.to_string(), Some(port_part.to_string()))
    } else {
        (ip.clone(), None)
    };

    let final_port = match (port, extracted_port) {
        (Some(p), _) => p,
        (None, Some(p)) => p,
        (None, None) => "80".to_string(),
    };

    let addr = format!("{}:{}", host, final_port);
    let socket_addr: SocketAddr = addr.parse()?;
    let stream = TcpStream::connect(socket_addr).await?;
    Ok(stream)
}

// Takes a regular tcp stream and converts it to a broadcast channel
// forwards the messages from the stream to a broadcast
pub async fn tcp_to_broadcast(stream: TcpStream) -> Sender<Vec<u8>> {
    let (tx, mut rx) = broadcast::channel::<Vec<u8>>(16);

    let (mut reader, mut writer) = stream.into_split();

    let mut broadcast_rx = rx.resubscribe();
    tokio::spawn(async move {
        while let Ok(msg) = broadcast_rx.recv().await {
            if let Err(e) = writer.write_all(&msg).await {
                eprintln!("[tcp_to_broadcast] Failed to write to socket: {}", e);
                break;
            }
        }
    });

    let tx_clone = tx.clone();
    tokio::spawn(async move {
        let mut buf = [0u8; 1024];
        loop {
            match reader.read(&mut buf).await {
                Ok(0) => break,
                Ok(n) => {
                    let _ = tx_clone.send(buf[..n].to_vec());
                }
                Err(e) => {
                    eprintln!("[tcp_to_broadcast] TCP read error: {}", e);
                    break;
                }
            }
        }
    });

    tx
}

// fn get_arg_or_env_var<T: std::str::FromStr>(env_var: &str, arg: Option<T>) -> Option<T> {
//     arg.or_else(|| env::var(env_var).ok().and_then(|s| s.parse().ok()))
// }

// Looks for a env varible, if its not found, try the specified default, if none is found it will use the default of whatever that type is
fn get_env_var_or_arg<T: std::str::FromStr>(env_var: &str, default: Option<T>) -> Option<T> {
    env::var(env_var)
        .ok()
        .and_then(|s| s.parse().ok())
        .or(default)
}

// Main function, entrypoint to the program, initalizes the app state, serves a tcp connection
// at the specified and does most of the intial handling of data, including switching between modes (json and file)
// and forwards some messages to other functions to handle command or console data, does health checks and set up the forwarding
// and re-attaching of the server stdin to go back to the main server
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let config_local_url = get_env_var_or_arg("LOCALURL", Some(StaticLocalUrl.to_string()));
    // let node_password: String = get_env_var_or_arg("NODE_PASSWORD", Some(String::default())).unwrap();

    //const PORT: u16 = 8082;
    //let listener = TcpListener::bind(format!("0.0.0.0:{}", PORT)).await?;
    let listener = TcpListener::bind(config_local_url.clone().unwrap()).await?;
    println!("Listening on {}", config_local_url.unwrap());

    let shared_stdin: Arc<Mutex<Option<ChildStdin>>> = Arc::new(Mutex::new(None));
    let hostname_ref: Arc<Result<OsString, String>> = Arc::new(match hostname::get() {
        Ok(h) => Ok(h),
        Err(e) => Err(e.to_string()),
    });

    let arc_db: Arc<Mutex<jsondatabase::Database>> = Arc::new(Mutex::new(load_db()));
    //let db = Arc::new(Mutex::new(load_db()));

    let mut state = AppState {
        current_server: None,
        server_running: Arc::new(Mutex::new(false)),
        server_output_tx: Arc::new(Mutex::new(None)),
        server_process: Arc::new(Mutex::new(None)),
        //server_index: HashMap::new(),
        db: Arc::clone(&arc_db),
    };

    //state.server_index = db.servers;
    //

    // TODO: remove this in favor of it being both written to a persistent db and insertions happening by the server
    // let db_clone = db
    let mut db = arc_db.lock().await;
    if !db.server_index.iter().any(|(name, _)| name == "minecraft"){
        db.server_index.insert("minecraft".to_string(), ServerIndex::new("server/".to_string(), "minecraft".to_string()));
        save_db(&db);
    }
    drop(db);

    let arc_state = Arc::new(state);

    let health_monitor_state = arc_state.clone();

    let mut kill_socket = false;
    const FILE_DELIMITER: &[u8] = b"<|END_OF_FILE|>";
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(5));
        loop {
            interval.tick().await;

            let server_running = health_monitor_state.server_running.lock().await;
            let mut server_process = health_monitor_state.server_process.lock().await;

            if *server_running {
                if let Some(process) = server_process.as_mut() {
                    match process.try_wait() {
                        Ok(Some(status)) => {
                            drop(server_running);
                            drop(server_process);

                            let mut server_running =
                                health_monitor_state.server_running.lock().await;
                            *server_running = false;

                            let mut server_process =
                                health_monitor_state.server_process.lock().await;
                            *server_process = None;

                            let mut output_tx = health_monitor_state.server_output_tx.lock().await;
                            *output_tx = None;

                            println!("Server state reset due to process exit");
                        }
                        Ok(None) => {}
                        Err(e) => {
                            eprintln!("Error checking server process: {}", e);
                        }
                    }
                } else {
                    drop(server_running);
                    drop(server_process);

                    let mut server_running = health_monitor_state.server_running.lock().await;
                    *server_running = false;

                    let mut output_tx = health_monitor_state.server_output_tx.lock().await;
                    *output_tx = None;
                }
            }
        }
    });

    {
        let mut server_running = arc_state.server_running.lock().await;
        *server_running = true;
    }

    {
        let mut server_running = arc_state.server_running.lock().await;
        *server_running = false;
    }

    loop {
        let (socket, addr) = listener.accept().await?;
        println!("[Connection] New client from {}", addr);

        debug_dump_state(&arc_state, &format!("New connection from {}", addr)).await;

        let stdin_ref = shared_stdin.clone();
        let hostname_ref = hostname_ref.clone();
        let arc_state_clone = arc_state.clone();

        tokio::spawn(async move {
            println!("[{}] DEBUG: Connection task started", addr);

            let (mut read_half, mut write_half) = socket.into_split();
            let (out_tx, mut out_rx) = mpsc::channel::<String>(128);
            let (cmd_tx, mut cmd_rx) = mpsc::channel::<String>(128);

            let mut server_output_rx = {
                let server_running_lock = arc_state_clone.server_running.lock().await;
                let output_tx_lock = arc_state_clone.server_output_tx.lock().await;

                if *server_running_lock {
                    if let Some(ref tx) = *output_tx_lock {
                        Some(tx.subscribe())
                    } else {
                        None
                    }
                } else {
                    None
                }
            };

            let arc_state_for_writer = arc_state_clone.clone();
            let addr_clone = addr.clone();
            tokio::spawn(async move {
                let mut retry_interval = tokio::time::interval(tokio::time::Duration::from_secs(2));
                let mut needs_server_status_check = server_output_rx.is_none();

                loop {
                    if needs_server_status_check {
                        let server_running_lock = arc_state_for_writer.server_running.lock().await;
                        let output_tx_lock = arc_state_for_writer.server_output_tx.lock().await;

                        if *server_running_lock {
                            if let Some(ref tx) = *output_tx_lock {
                                server_output_rx = Some(tx.subscribe());
                                needs_server_status_check = false;

                                let connection_msg = serde_json::json!({
                                    "type": "info",
                                    "data": "Connected to server output stream",
                                    "authcode": "0"
                                })
                                .to_string()
                                    + "\n";

                                if let Err(e) =
                                    write_half.write_all(connection_msg.as_bytes()).await
                                {
                                    eprintln!("[{}] Write error: {}", addr_clone, e);
                                    break;
                                }
                            }
                        }
                    }

                    tokio::select! {
                        Some(msg) = cmd_rx.recv() => {
                            let payload = serde_json::json!({
                                "type": "info",
                                "data": msg,
                                "authcode": "0"
                            }).to_string() + "\n";
                            if let Err(e) = write_half.write_all(payload.as_bytes()).await {
                                eprintln!("[{}] Write error: {}", addr_clone, e);
                                break;
                            }
                        }

                        Some(out) = out_rx.recv() => {
                            if let Err(e) = write_half.write_all((out + "\n").as_bytes()).await {
                                eprintln!("[{}] Write error: {}", addr_clone, e);
                                break;
                            }
                        }

                        server_msg = async {
                            if let Some(rx) = &mut server_output_rx {
                                rx.recv().await
                            } else {
                                retry_interval.tick().await;
                                Err(broadcast::error::RecvError::Closed)
                            }
                        } => {
                            match server_msg {
                                Ok(msg) => {
                                    if let Err(e) = write_half.write_all((msg + "\n").as_bytes()).await {
                                        eprintln!("[{}] Write error: {}", addr_clone, e);
                                        break;
                                    }
                                }
                                Err(broadcast::error::RecvError::Lagged(_)) => {
                                    println!("[{}] Lagged behind server output, catching up", addr_clone);
                                    continue;
                                }
                                Err(broadcast::error::RecvError::Closed) => {
                                    needs_server_status_check = true;
                                }
                            }
                        }

                        _ = tokio::time::sleep(tokio::time::Duration::from_millis(100)) => {
                        }
                    }
                }
            });
            let mut read_buf = Vec::new();
            let mut temp_buf = [0u8; 20632];

            enum ReadMode {
                Json,
                File {
                    current_file: tokio::fs::File,
                    file_name: String,
                    bytes_written: u64,
                    last_logged_mb: u64,
                    last_activity: tokio::time::Instant,
                },
            }

            let mut mode = ReadMode::Json;
            let mut files_received = 0;

            let mut tick = tokio::time::interval(tokio::time::Duration::from_secs(1));
            tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);

            loop {
                if kill_socket == true {
                    println!("Shutting down");
                    //write_half.shutdown();
                    kill_socket = false;
                    break;
                }
                tokio::select! {
                    result = read_half.read(&mut temp_buf) => {
                        let n = match result {
                            Ok(0) => break,
                            Ok(n) => n,
                            Err(e) => {
                                eprintln!("[{}] Read error: {}", addr, e);
                                break;
                            }
                        };
                        read_buf.extend_from_slice(&temp_buf[..n]);
                    }

                    _ = tick.tick() => {}

                    _ = tokio::time::sleep(tokio::time::Duration::from_secs(120)) => {
                        continue;
                    }
                }

                const MAX_BUFFER_SIZE: usize = 50 * 1024 * 1024;
                if read_buf.len() > MAX_BUFFER_SIZE {
                    read_buf.clear();
                    mode = ReadMode::Json;
                    continue;
                }

                loop {

                    match &mut mode {
                        ReadMode::Json => {
                            let mut found_message = false;

                            while let Some(newline_pos) = read_buf.iter().position(|&b| b == b'\n')
                            {
                                let line = &read_buf[..newline_pos];

                                if line.is_empty() {
                                    if newline_pos + 1 <= read_buf.len() {
                                        read_buf.drain(..newline_pos + 1);
                                    } else {
                                        read_buf.clear();
                                    }
                                    continue;
                                }

                                let line_str = String::from_utf8_lossy(line);

                                if line_str.trim() == "<|END_OF_FILE|>" {
                                    if newline_pos + 1 <= read_buf.len() {
                                        read_buf.drain(..newline_pos + 1);
                                    } else {
                                        read_buf.clear();
                                    }
                                    found_message = true;
                                    continue;
                                }
                                // else if let Ok(msg_payload) =
                                //     serde_json::from_value::<IntergrationCommands>(
                                //         json_value.clone(),
                                //     ){
                                //     println!("[{}] Successfully parsed as Value: {:#?}", addr, json_value);
                                //     sort_command_type_or_console(
                                //         &Arc::clone(&arc_state_clone),
                                //         &serde_json::to_value(msg_payload).unwrap(),
                                //         &out_tx,
                                //         &cmd_tx,
                                //         &stdin_ref,
                                //         &hostname_ref,
                                //     ).await;
                                // }
                                if line_str.trim().starts_with('{')
                                    && line_str.trim().ends_with('}')
                                {
                                    // println!("[{}] Received JSON line: {}", addr, line_str.trim());
                                    if let Ok(json_value) = serde_json::from_slice::<Value>(line) {
                                        if let Ok(request) =
                                            serde_json::from_value::<FileRequestMessage>(
                                                json_value.clone(),
                                            )
                                        {
                                            let out_tx_clone = out_tx.clone();
                                            let arc_state_for_spawn = arc_state_clone.clone();
                                            tokio::spawn(async move {
                                                let response_json = handle_file_request(
                                                    &Arc::clone(&arc_state_for_spawn),
                                                    request,
                                                )
                                                .await;
                                                let _ = out_tx_clone.send(response_json).await;
                                            });
                                        } else if let Ok(payload) =
                                            serde_json::from_value::<SrcAndDest>(json_value.clone())
                                        {
                                            if let ApiCalls::Node(dest) = payload.dest {
                                                match unsure_ip_or_port_tcp_conn(
                                                    Some(dest.ip.clone()),
                                                    None,
                                                )
                                                .await
                                                {
                                                    Ok(conn) => {
                                                        let writer_tx = tcp_to_writer(conn).await;
                                                        tokio::spawn(async move {
                                                            let _ = send_folder_over_broadcast(
                                                                "server/", writer_tx,
                                                            )
                                                            .await;
                                                        });
                                                    }
                                                    Err(e) => eprintln!(
                                                        "[{}] Failed to connect: {}",
                                                        addr, e
                                                    ),
                                                }
                                            } else {
                                                sort_command_type_or_console(
                                                    &Arc::clone(&arc_state_clone),
                                                    &json_value,
                                                    &out_tx,
                                                    &cmd_tx,
                                                    &stdin_ref,
                                                    &hostname_ref,
                                                )
                                                .await;
                                            }
                                        } else if let Ok(msg_payload) =
                                            serde_json::from_value::<IncomingMessageWithMetadata>(
                                                json_value.clone(),
                                            )
                                        {
                                            println!(
                                                "[{}] DEBUG: Processing command with metadata: {}",
                                                addr, msg_payload.message
                                            );
                                            handle_commands_with_metadata(
                                                arc_state_clone.clone(),
                                                &msg_payload,
                                                &cmd_tx,
                                                &stdin_ref,
                                                &hostname_ref,
                                            )
                                            .await;
                                        }
                                        //  else if let Ok(msg_payload) =
                                        //     serde_json::from_value::<IntergrationCommands>(
                                        //         json_value.clone(),
                                        //     )
                                        // {
                                        //     //println!("[{}] Successfully parsed as Value: {:#?}", addr, json_value);
                                        //     sort_command_type_or_console(
                                        //         &Arc::clone(&arc_state_clone),
                                        //         &serde_json::to_value(msg_payload).unwrap(),
                                        //         &out_tx,
                                        //         &cmd_tx,
                                        //         &stdin_ref,
                                        //         &hostname_ref,
                                        //     )
                                        //     .await;
                                        // }
                                        else if let Ok(msg_payload) =
                                            serde_json::from_value::<MessagePayload>(
                                                json_value.clone(),
                                            )
                                        {
                                            match msg_payload.r#type.as_str() {
                                                "start_file" => {
                                                    files_received += 1;
                                                    println!(
                                                        "[File Transfer] {} is being transferred",
                                                        msg_payload.message
                                                    );
                                                    let file_path =
                                                        format!("server/{}", msg_payload.message);
                                                    let _ = tokio::fs::create_dir_all(
                                                        file_path.clone(),
                                                    )
                                                    .await;

                                                    if let Some(parent) =
                                                        std::path::Path::new(&file_path).parent()
                                                    {
                                                        let _ =
                                                            tokio::fs::create_dir_all(parent).await;
                                                    }

                                                    if let Ok(file) = tokio::fs::OpenOptions::new()
                                                        .create(true)
                                                        .write(true)
                                                        .truncate(true)
                                                        .open(&file_path)
                                                        .await
                                                    {
                                                        mode = ReadMode::File {
                                                            current_file: file,
                                                            file_name: msg_payload.message.clone(),
                                                            bytes_written: 0,
                                                            last_logged_mb: 0,
                                                            last_activity:
                                                                tokio::time::Instant::now(),
                                                        };
                                                        if newline_pos + 1 <= read_buf.len() {
                                                            read_buf.drain(..newline_pos + 1);
                                                        } else {
                                                            read_buf.clear();
                                                        }
                                                        found_message = true;
                                                        break;
                                                    }
                                                }
                                                "end_file" => {
                                                    if newline_pos + 1 <= read_buf.len() {
                                                        read_buf.drain(..newline_pos + 1);
                                                    } else {
                                                        read_buf.clear();
                                                    }
                                                    found_message = true;
                                                    continue;
                                                }
                                                "clean_file" => {
                                                    let file_path =
                                                        format!("server/{}", msg_payload.message);
                                                    if tokio::fs::metadata(&file_path).await.is_ok()
                                                    {
                                                        let _ = cleanup_end_file_markers(
                                                            &file_path,
                                                            &msg_payload.message,
                                                        )
                                                        .await;
                                                    }
                                                    if newline_pos + 1 <= read_buf.len() {
                                                        read_buf.drain(..newline_pos + 1);
                                                    } else {
                                                        read_buf.clear();
                                                    }
                                                    found_message = true;
                                                    continue;
                                                }
                                                "command" => {
                                                    println!("{} {}", addr, msg_payload.message);
                                                    if msg_payload.message == "start_server" {
                                                        debug_dump_state(
                                                            &arc_state_clone,
                                                            &format!(
                                                                "Before start_server from {}",
                                                                addr
                                                            ),
                                                        )
                                                        .await;

                                                        if let Err(e) = start_server_with_broadcast(
                                                            &arc_state_clone,
                                                            &stdin_ref,
                                                            &cmd_tx,
                                                        )
                                                        .await
                                                        {
                                                            eprintln!(
                                                                "[{}] Failed to start server: {}",
                                                                addr, e
                                                            );
                                                        }

                                                        debug_dump_state(
                                                            &arc_state_clone,
                                                            &format!(
                                                                "After start_server from {}",
                                                                addr
                                                            ),
                                                        )
                                                        .await;
                                                    } else {
                                                        // handle_typical_command_or_console(
                                                        //     &Arc::clone(&arc_state_clone),
                                                        //     &msg_payload,
                                                        //     &out_tx,
                                                        //     &cmd_tx,
                                                        //     &stdin_ref,
                                                        //     &hostname_ref,
                                                        // )
                                                        // .await;
                                                        // println!("{:#?}", msg_payload);
                                                        sort_command_type_or_console(
                                                            &Arc::clone(&arc_state_clone),
                                                            &serde_json::to_value(msg_payload)
                                                                .unwrap(),
                                                            &out_tx,
                                                            &cmd_tx,
                                                            &stdin_ref,
                                                            &hostname_ref,
                                                        )
                                                        .await;
                                                    }
                                                }
                                                _ => {
                                                    // println!("{:#?}", msg_payload);
                                                    sort_command_type_or_console(
                                                        &Arc::clone(&arc_state_clone),
                                                        &serde_json::to_value(msg_payload).unwrap(),
                                                        &out_tx,
                                                        &cmd_tx,
                                                        &stdin_ref,
                                                        &hostname_ref,
                                                    )
                                                    .await;
                                                }
                                            }
                                        } else {
                                            let command_or_console_result = sort_command_type_or_console(
                                                &Arc::clone(&arc_state_clone),
                                                &json_value,
                                                &out_tx,
                                                &cmd_tx,
                                                &stdin_ref,
                                                &hostname_ref,
                                            )
                                            .await;
                                            if let Err(e) = command_or_console_result {
                                                if let Some(CommandOrConsoleErrors::AuthDisconnect) = e.downcast_ref::<CommandOrConsoleErrors>() {
                                                    kill_socket = true;
                                                    // let _ = socket.shutdown().await;
                                                    //drop(socket);
                                                }
                                            }

                                        }
                                        //          else if let Ok(msg_payload) =
                                        //     serde_json::from_value::<IntergrationCommands>(
                                        //         json_value.clone(),
                                        //     )
                                        // {
                                        //     //println!("[{}] Successfully parsed as Value: {:#?}", addr, json_value);
                                        //     sort_command_type_or_console(
                                        //         &Arc::clone(&arc_state_clone),
                                        //         &serde_json::to_value(msg_payload).unwrap(),
                                        //         &out_tx,
                                        //         &cmd_tx,
                                        //         &stdin_ref,
                                        //         &hostname_ref,
                                        //     )
                                        //     .await;
                                        // }
                                        // else if let Ok(msg_payload) =
                                        //     serde_json::from_value::<IntergrationCommands>(
                                        //         json_value.clone(),
                                        //     ){
                                        //     println!("[{}] Successfully parsed as Value: {:#?}", addr, json_value);
                                        //     sort_command_type_or_console(
                                        //         &Arc::clone(&arc_state_clone),
                                        //         &serde_json::to_value(msg_payload).unwrap(),
                                        //         &out_tx,
                                        //         &cmd_tx,
                                        //         &stdin_ref,
                                        //         &hostname_ref,
                                        //     ).await;
                                        // }
                                    } else {
                                        break;
                                    }
                                } else {
                                    break;
                                }

                                if newline_pos + 1 <= read_buf.len() {
                                    read_buf.drain(..newline_pos + 1);
                                    found_message = true;
                                } else {
                                    read_buf.clear();
                                    break;
                                }
                            }

                            if !found_message {
                                break;
                            }
                        }

                        ReadMode::File {
                            current_file,
                            file_name,
                            bytes_written,
                            last_logged_mb,
                            last_activity,
                        } => {
                            *last_activity = tokio::time::Instant::now();

                            if let Some(delim_pos) = find_subsequence(&read_buf, FILE_DELIMITER) {
                                if delim_pos > 0 {
                                    let _ = current_file.write_all(&read_buf[..delim_pos]).await;
                                    *bytes_written += delim_pos as u64;
                                }

                                let _ = current_file.flush().await;
                                let file_path = format!("server/{}", file_name);
                                let _ = cleanup_end_file_markers(&file_path, file_name).await;

                                let drain_end = delim_pos + FILE_DELIMITER.len();
                                if drain_end <= read_buf.len() {
                                    read_buf.drain(..drain_end);
                                } else {
                                    read_buf.clear();
                                }

                                mode = ReadMode::Json;
                                continue;
                            }

                            let keep_buffer_size = FILE_DELIMITER.len() + 1;
                            if read_buf.len() > keep_buffer_size {
                                let write_size = read_buf.len() - keep_buffer_size;
                                let _ = current_file.write_all(&read_buf[..write_size]).await;
                                *bytes_written += write_size as u64;
                                read_buf.drain(..write_size);
                            } else {
                                break;
                            }
                        }
                    }
                }
            }
        });
    }
}
#[derive(Debug)]
enum CommandOrConsoleErrors {
    AuthDisconnect,
}
impl std::error::Error for CommandOrConsoleErrors {}
impl fmt::Display for CommandOrConsoleErrors {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CommandOrConsoleErrors::AuthDisconnect => write!(f, "Authentication disconnected"),
        }
    }
}
#[derive(Debug, serde::Serialize, serde::Deserialize, Clone)]
struct AuthTcpMessage {
    password: String,
}

// TODO: merge with handle_typical_command_or_console
// At the time of writing this, i am working on getting intergration commands to work here
// on the node, I did a sub-optimal solution for this command type which is, since regular
// message payloads expect strings and immediately serializes into a structure that doesnt represent
// how I want IntergrationCommands to work, I decided to make this function for the time being to ensure the
// commands are handled properly and serialized properly, as well as other commands be forwarded to
// the relevent function.
// This now also handles auth messages
// TODO: Merge alot of the functionality back into main at some point
// TODO: remove excessive Ok's and have proper error handling for cases which should not return OK
async fn sort_command_type_or_console(
    arc_state: &Arc<AppState>,
    payload: &serde_json::Value,
    out_tx: &mpsc::Sender<String>,
    cmd_tx: &mpsc::Sender<String>,
    stdin_ref: &Arc<Mutex<Option<ChildStdin>>>,
    hostname: &Arc<Result<OsString, String>>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    //node_password
    let node_password: String =
        get_env_var_or_arg("NODE_PASSWORD", Some(String::default())).unwrap();

    let standard_command_payload_result: Result<MessagePayload, serde_json::Error> =
        serde_json::from_value(payload.clone());
    if let Ok(standard_command_payload) = standard_command_payload_result {
        handle_typical_command_or_console(
            &arc_state,
            &standard_command_payload,
            &out_tx,
            &cmd_tx,
            &stdin_ref,
            &hostname,
        )
        .await;
        //Ok(())
    }
    //let dest = request.dest.as_inner_str();
    let file_operation_result: Result<SrcAndDest, serde_json::Error> = 
        serde_json::from_value(payload.clone());
    if let Ok(file_operation) = file_operation_result { 
        let (converted_src, converted_dest): (FileOperations, FileOperations) = (file_operation.clone().src.into(), file_operation.clone().dest.into());
        let state = Arc::clone(arc_state);
        let mut option_path = get_definite_path_from_name(
            &state,
            get_provider_from_servername(&state, Some(state.current_server.clone().ok_or("there is no current server")?)).await,
        )
        .await;
        
        if let Some(mut path) = option_path {
            if !path.starts_with("server/") {
                path = format!("server/{}", path);
            }

            execute_file_operation(converted_src, converted_dest, path);
        }
    }

    let auth_payload_result: Result<AuthTcpMessage, serde_json::Error> =
        serde_json::from_value(payload.clone());
    if let Ok(auth_payload) = auth_payload_result {
        if node_password != auth_payload.password {
            println!("Authentication failed");
            return Err(Box::new(CommandOrConsoleErrors::AuthDisconnect));
        }
        // if node_password == auth_payload.password {
        //     Ok(())
        // } else {
        //     Err()
        // }
    }

    let intergration_command_payload_result: Result<IntergrationCommands, serde_json::Error> =
        serde_json::from_value(payload.clone());
    if let Ok(intergration_command_payload) = intergration_command_payload_result {
        run_intergration_commands(intergration_command_payload).await;
        //Ok(())
    }
    Ok(())
}

// Handles either commands or console output, should eventually be replaced by handle_commands_with_metadata
// and eventually there should be out_tx added to it. The commands are mainly related to server management, like deleting the server, (delete the files)
// stopping it (TODO: stop isnt the universal keyword to stop all servers, fix that and make it depend on the provider)
// console output is forwarded directly to the server process via channel
async fn handle_typical_command_or_console(
    arc_state: &Arc<AppState>,
    payload: &MessagePayload,
    out_tx: &mpsc::Sender<String>,
    cmd_tx: &mpsc::Sender<String>,
    stdin_ref: &Arc<Mutex<Option<ChildStdin>>>,
    hostname: &Arc<Result<OsString, String>>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let typ = payload.r#type.clone();
    let state = Arc::clone(arc_state);
    if typ == "command" {
        let cmd_str = payload.message.clone();
        match cmd_str.as_str() {
            "delete_server" => {
                let mut option_path = get_definite_path_from_name(
                    &state,
                    get_provider_from_servername(&state, Some(state.current_server.clone().ok_or("there is no current server")?)).await,
                )
                .await;
                if let Some(mut path) = option_path { 
                    if !path.starts_with("server/") {
                        path = format!("server/{}", path);
                    }
                    if let Err(errro) = fs::remove_dir_all(&path).await {
                        eprintln!("Failed to delete directory {}: {}", path, errro);
                        Ok(())
                    } else {
                        if let Err(errro) = fs::create_dir(&path).await {
                            eprintln!("Failed to recreate directory {}: {}", path, errro);
                            Ok(())
                        } else {
                            println!("Successfully cleared directory: {}", path);
                            Ok(())
                        }
                    }
                } else {
                    Ok(())
                }
            }
            "server_state" => {
                //println!("Sending back state");
                let status = &state.server_running.lock().await;
                //println!("{:#?}", status);
                let status_message = MessagePayload {
                    r#type: "server_state".to_string(),
                    message: status.to_string(),
                    authcode: "0".to_string(),
                };
                //println!("{:#?}", status_message);
                let json_str = serde_json::to_string(&status_message).unwrap();
                out_tx.send(json_str).await;
                Ok(())
            }
            "stop_server" => {
                if let Some(prov) = get_provider_object(
                    get_provider_from_servername(&state, Some(state.current_server.clone().ok_or("there is no current server")?)).await.as_deref(),
                    get_definite_path_from_name(
                        &state,
                        get_provider_from_servername(&state, Some(state.current_server.clone().ok_or("there is no current server")?)).await,
                    )
                    .await.as_deref(),
                ) {
                    let input = "stop";
                    let mut guard = stdin_ref.lock().await;
                    if let Some(stdin) = guard.as_mut() {
                        let _ = stdin.write_all(format!("{}\n", input).as_bytes()).await;
                        let _ = stdin.flush().await;
                        let _ = cmd_tx.send(format!("Sent to server: {}", input)).await;
                        Ok(())
                    } else {
                        Ok(())
                    }
                } else {
                    Ok(())
                }
            }
            "start_server" => {
                {
                    let stdin_guard = stdin_ref.lock().await;
                    if stdin_guard.is_some() {
                        let _ = cmd_tx
                            .send("Server is already running. Use 'stop_server' first.".into())
                            .await;
                        return Ok(());
                    }
                }

                if let Some(prov) = get_provider_object(
                    get_provider_from_servername(&state, Some(state.current_server.clone().ok_or("there is no current server")?)).await.as_deref(),
                    get_definite_path_from_name(
                        &state,
                        get_provider_from_servername(&state, Some(state.current_server.clone().ok_or("there is no current server")?)).await,
                    )
                    .await.as_deref(),
                ) {
                    if let Some(cmd) = prov.start() {
                        let tx = cmd_tx.clone();
                        let stdin_clone = stdin_ref.clone();

                        tokio::spawn(async move {
                            let result = run_command_live_output(
                                &state,
                                cmd,
                                "Server".into(),
                                Some(tx.clone()),
                                Some(stdin_clone.clone()),
                            )
                            .await;
                            {
                                let mut stdin_guard = stdin_clone.lock().await;
                                *stdin_guard = None;
                            }

                            match result {
                                Ok(_) => {
                                    let _ = tx.send("Server process ended".into()).await;
                                }
                                Err(e) => {
                                    let _ = tx.send(format!("Server process failed: {}", e)).await;
                                }
                            }
                        });

                        let _ = cmd_tx.send("Server started".into()).await;
                        Ok(())
                    } else {
                        let _ = cmd_tx
                            .send("No start command available for this provider".into())
                            .await;
                        Ok(())
                    }
                } else {
                    let _ = cmd_tx
                        .send("Failed to get provider for server".into())
                        .await;
                    Ok(())
                }
            }
            "server_data" => {
                let _ = cmd_tx
                    .send(
                        serde_json::to_string(&GetState {
                            start_keyword: "help".to_string(),
                            stop_keyword: "All dimensions are saved".to_string(),
                        })
                        .unwrap(),
                    )
                    .await;
                Ok(())
            }
            "server_name" => {
                let hostname_str = match hostname.as_ref() {
                    Ok(os) => os.to_string_lossy().to_string(),
                    Err(e) => e.clone(),
                };
                let _ = cmd_tx
                    .send(
                        serde_json::to_string(&MessagePayload {
                            r#type: "command".to_string(),
                            message: hostname_str,
                            authcode: "0".to_string(),
                        })
                        .unwrap(),
                    )
                    .await;
                Ok(())
            }
            other => {
                let _ = cmd_tx.send(format!("Unknown command: {}", other)).await;
                Ok(())
            }
        }
    } else if typ == "console" {
        let input = payload.message.clone();
        let mut guard = stdin_ref.lock().await;
        if let Some(stdin) = guard.as_mut() {
            let _ = stdin.write_all(format!("{}\n", input).as_bytes()).await;
            let _ = stdin.flush().await;
            let _ = cmd_tx.send(format!("Sent to server: {}", input)).await;
            Ok(())
        } else {
            Ok(())
        }
    } else {
        Ok(())
    }
}

// TODO: this was for debugging, remove this
async fn debug_dump_state(state: &Arc<AppState>, label: &str) {
    // let server_running = state.server_running.lock().await;
    // let output_tx_lock = state.server_output_tx.lock().await;
    // let process_lock = state.server_process.lock().await;

    // println!("DEBUG DUMP [{}]:", label);
    // println!("  - server_running: {}", *server_running);
    // println!("  - output_tx exists: {}", output_tx_lock.is_some());
    // println!("  - process exists: {}", process_lock.is_some());
    // println!("  - current_server: {}", state.current_server);

    // if let Some(ref tx) = *output_tx_lock {
    //     println!("  - broadcast receiver_count: {}", tx.receiver_count());
    // }
}

// starts the server with the channel (broadcast) in which it will receive and send out commands (for the server, not server management commands)
async fn start_server_with_broadcast(
    state: &Arc<AppState>,
    shared_stdin: &Arc<Mutex<Option<ChildStdin>>>,
    cmd_tx: &mpsc::Sender<String>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    {
        let server_running = state.server_running.lock().await;
        if *server_running {
            return Ok(());
        }
    }

    {
        let mut process_lock = state.server_process.lock().await;
        if let Some(mut child) = process_lock.take() {
            let _ = child.kill().await;
        }
    }

    {
        let mut output_tx_lock = state.server_output_tx.lock().await;
        *output_tx_lock = None;
    }

    let (broadcast_tx, _) = broadcast::channel(1000);

    println!("Adjusted path to: server/");
    if let Some(provider_type) = get_provider_object(
        get_provider_from_servername(&state, Some(state.current_server.clone().ok_or("there is no current server")?)).await.as_deref(),
        get_definite_path_from_name(
            state,
            Some(state.current_server.clone().ok_or("there is no current server")?)
            //get_provider_from_servername(&state, state.current_server.clone()).await,
        )
        .await.as_deref(),
    ) {
        println!("Selected provider: {}", provider_type.name);

        // if let Some(pre_hook_cmd) = provider_type.pre_hook() {
        //     let mut cmd = pre_hook_cmd;
        //     cmd.current_dir("server/");
        //     let _ = run_command_live_output(
        //         cmd,
        //         "Pre-hook".into(),
        //         Some(cmd_tx.clone()),
        //         None,
        //     )
        //     .await;
        // }

        let start_command = provider_type
            .start()
            .ok_or("Provider does not support starting servers")?;

        let mut child = tokio::process::Command::from(start_command)
            .current_dir("server/")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;

        let stdin = child.stdin.take().expect("Failed to open stdin");
        let stdout = child.stdout.take().expect("Failed to open stdout");
        let stderr = child.stderr.take().expect("Failed to open stderr");

        {
            let mut shared_stdin_lock = shared_stdin.lock().await;
            *shared_stdin_lock = Some(stdin);
        }

        {
            let mut process_lock = state.server_process.lock().await;
            *process_lock = Some(child);
        }

        {
            let mut output_tx_lock = state.server_output_tx.lock().await;
            *output_tx_lock = Some(broadcast_tx.clone());
        }

        {
            let mut server_running = state.server_running.lock().await;
            *server_running = true;
        }

        // if let Some(post_hook_cmd) = provider_type.post_hook() {
        //     let cmd_tx_clone = cmd_tx.clone();
        //     tokio::spawn(async move {
        //         let mut cmd = post_hook_cmd;
        //         cmd.current_dir("server/");
        //         let _ = run_command_live_output(
        //             cmd,
        //             "Post-hook".into(),
        //             Some(cmd_tx_clone),
        //             None,
        //         )
        //         .await;
        //     });
        // }

        let broadcast_tx_clone = broadcast_tx.clone();
        tokio::spawn(async move {
            let mut stdout_reader = BufReader::new(stdout);
            let mut line = String::new();
            while stdout_reader.read_line(&mut line).await.is_ok() && !line.is_empty() {
                let output_msg = serde_json::json!({
                    "type": "info",
                    "data": serde_json::json!({
                        "type": "stdout",
                        "data": line.trim()
                    }).to_string(),
                    "authcode": "0"
                })
                .to_string();

                let _ = broadcast_tx_clone.send(output_msg);
                line.clear();
            }
        });

        let broadcast_tx_clone = broadcast_tx.clone();
        tokio::spawn(async move {
            let mut stderr_reader = BufReader::new(stderr);
            let mut line = String::new();
            while stderr_reader.read_line(&mut line).await.is_ok() && !line.is_empty() {
                let output_msg = serde_json::json!({
                    "type": "info",
                    "data": serde_json::json!({
                        "type": "stderr",
                        "data": line.trim()
                    }).to_string(),
                    "authcode": "0"
                })
                .to_string();

                let _ = broadcast_tx_clone.send(output_msg);
                line.clear();
            }
        });
    }
    Ok(())
}

// Dedicated function for stopping the server, There was already have something like this in a match statement but this function is mroe recent and handles the process, in the
// match statement I should invoke this function instead of having its own logic in the match statement
// TODO: do the above
async fn stop_server(
    state: &Arc<AppState>,
    shared_stdin: &Arc<Mutex<Option<ChildStdin>>>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let mut server_running = state.server_running.lock().await;
    if !*server_running {
        return Ok(());
    }

    {
        let mut stdin_lock = shared_stdin.lock().await;
        if let Some(ref mut stdin) = *stdin_lock {
            let _ = stdin.write_all(b"stop\n").await;
        }
    }

    tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;

    {
        let mut process_lock = state.server_process.lock().await;
        if let Some(mut child) = process_lock.take() {
            let _ = child.kill().await;
        }
    }

    {
        let mut stdin_lock = shared_stdin.lock().await;
        *stdin_lock = None;
    }

    {
        let mut output_tx_lock = state.server_output_tx.lock().await;
        *output_tx_lock = None;
    }

    *server_running = false;
    Ok(())
}


async fn fix_path(path: String) -> String {
    let server_root = Path::new("server");

    if path.starts_with("server/") || path == "server" {
        let canonical = fs::canonicalize(&path)
            .await.unwrap_or_else(|_| server_root.to_path_buf());

        let canonical_server_root = fs::canonicalize(server_root)
            .await.unwrap_or_else(|_| server_root.to_path_buf());

        if canonical.starts_with(&canonical_server_root) {
            return canonical.to_string_lossy().into_owned();
        }

        let fixed = server_root.join(path.trim_start_matches("server/"));
        return fixed.to_string_lossy().into_owned();
    }

    let forced = server_root.join(path);

    let canonical_forced = fs::canonicalize(&forced)
        .await.unwrap_or(forced);

    canonical_forced.to_string_lossy().into_owned()
}

// Handles the file requests via easy match statement, easy for if i need it for another aspect of the whole gameserver stack
// Metadata just gives the metadata from a individual file
// PathFromTag will take a tag, usually coorosponding to a servers name or unique identifier, and return a path,
// as at some point it would be benifical if gameserver-rs could run
// servers from some nested directory so you dont need to migrate server files, delete it, then migrate newer server files
// or recreate it
// should be here and not filesystem because it contains appstate
async fn handle_file_request(state: &Arc<AppState>, request: FileRequestMessage) -> String {
    match request.payload {
        FileRequestPayload::Metadata { path } => match get_metadata(&fix_path(path).await).await {
            Ok(metadata) => serde_json::to_string(&FileResponseMessage {
                in_response_to: request.id,
                data: serde_json::to_value(metadata).unwrap(),
            })
            .unwrap(),
            Err(e) => serde_json::to_string(&FileResponseMessage {
                in_response_to: request.id,
                data: serde_json::json!({ "error": e.to_string() }),
            })
            .unwrap(),
        },
        FileRequestPayload::PathFromTag { tag, path } => {
            let path = fix_path(path).await;
            let basic_path_response = BasicPath { paths: vec![] };
            serde_json::to_string(&FileResponseMessage {
                in_response_to: request.id,
                data: serde_json::to_value(basic_path_response).unwrap(),
            })
            .unwrap()
        }
        FileRequestPayload::ListDir { path } => match list_directory(&fix_path(path).await).await {
            Ok(entries) => serde_json::to_string(&FileResponseMessage {
                in_response_to: request.id,
                data: serde_json::to_value(entries).unwrap(),
            })
            .unwrap(),
            Err(e) => serde_json::to_string(&FileResponseMessage {
                in_response_to: request.id,
                data: serde_json::json!({ "error": e.to_string() }),
            })
            .unwrap(),
        },
        FileRequestPayload::ListDirWithRange { path, start, end } => {
            match list_directory_with_range(&fix_path(path).await, start, end).await {
                Ok(entries) => serde_json::to_string(&FileResponseMessage {
                    in_response_to: request.id,
                    data: serde_json::to_value(entries).unwrap(),
                })
                .unwrap(),
                Err(e) => serde_json::to_string(&FileResponseMessage {
                    in_response_to: request.id,
                    data: serde_json::json!({ "error": e.to_string() }),
                })
                .unwrap(),
            }
        }
        FileRequestPayload::FileChunk(file_chunk) => match get_files_content(file_chunk).await {
            Ok(content_msg) => serde_json::to_string(&FileResponseMessage {
                in_response_to: request.id,
                data: serde_json::Value::String(content_msg.message),
            })
            .unwrap(),
            Err(e) => serde_json::to_string(&FileResponseMessage {
                in_response_to: request.id,
                data: serde_json::json!({ "error": e.to_string() }),
            })
            .unwrap(),
        },
    }
}

// this is a function which will look for a small slice (needle) in a bigger slice (haystack)
// if it finds it, it will return where it starts, otherwise returns None
fn find_subsequence(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.is_empty() || haystack.len() < needle.len() {
        return None;
    }

    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

// this function takes a tcp stream and forwards the data from that to the sender it returns, used a few times
pub async fn tcp_to_writer(stream: TcpStream) -> mpsc::Sender<Vec<u8>> {
    let (tx, mut rx) = mpsc::channel::<Vec<u8>>(1024);

    let (_reader, mut writer) = stream.into_split();

    tokio::spawn(async move {
        let mut total_bytes_written = 0u64;
        let mut message_count = 0u64;

        while let Some(msg) = rx.recv().await {
            message_count += 1;
            let msg_len = msg.len();

            match writer.write_all(&msg).await {
                Ok(()) => {
                    total_bytes_written += msg_len as u64;

                    // if message_count % 1000 == 0 || total_bytes_written % 10_000_000 == 0 {
                    //     println!(
                    //         "[tcp_to_writer] Wrote {} messages, {} total bytes",
                    //         message_count, total_bytes_written
                    //     );
                    // }
                }
                Err(e) => {
                    // eprintln!("[tcp_to_writer] Failed to write message {} ({} bytes) to socket after {} total bytes: {}",
                    //         message_count, msg_len, total_bytes_written, e);
                    break;
                }
            }

            if message_count % 100 == 0 {
                if let Err(e) = writer.flush().await {
                    eprintln!("[tcp_to_writer] Failed to flush socket: {}", e);
                    break;
                }
            }
        }

        if let Err(e) = writer.flush().await {
            eprintln!("[tcp_to_writer] Failed final flush: {}", e);
        }

        println!(
            "[tcp_to_writer] Writer task exiting after {} messages and {} bytes",
            message_count, total_bytes_written
        );
    });

    tx
}

// More modern version of handle_typical_command_or_console, except currently it only handles commands, mainly this is used to the singular command which requires a
// metadata feild (server data), to create a server
// TODO: eventually replace handle_typical_command_or_console with this
async fn handle_commands_with_metadata(
    state: Arc<AppState>,
    payload: &IncomingMessageWithMetadata,
    cmd_tx: &mpsc::Sender<String>,
    stdin_ref: &Arc<Mutex<Option<ChildStdin>>>,
    hostname: &Arc<Result<OsString, String>>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let typ = payload.message_type.clone();
    if typ == "command" {
        let cmd_str = payload.message.clone();
        match cmd_str.as_str() {
            "create_server" => {
                if let MetadataTypes::Server {
                    providertype,
                    location,
                    provider,
                    servername,
                } = &payload.metadata.clone()
                {
                    if let Some(prov) = get_provider_object(
                        Some(providertype),
                        get_definite_path_from_name(
                            &state,
                            get_provider_from_servername(&state, Some(state.current_server.clone().ok_or("there is no current server")?))
                                .await,
                        )
                        .await.as_deref(),
                    ) {
                        if let Some(cmd) = prov.pre_hook() {
                            run_command_live_output(
                                &state,
                                cmd,
                                "Pre-hook".into(),
                                Some(cmd_tx.clone()),
                                None,
                            )
                            .await
                            .ok();
                        }
                        if let Some(cmd) = prov.install() {
                            run_command_live_output(
                                &state,
                                cmd,
                                "Install".into(),
                                Some(cmd_tx.clone()),
                                None,
                            )
                            .await
                            .ok();
                        }
                        if let Some(cmd) = prov.post_hook() {
                            run_command_live_output(
                                &state,
                                cmd,
                                "Post-hook".into(),
                                Some(cmd_tx.clone()),
                                None,
                            )
                            .await
                            .ok();
                        }
                        if let Some(cmd) = prov.start() {
                            let tx = cmd_tx.clone();
                            let stdin_clone = stdin_ref.clone();
                            tokio::spawn(async move {
                                run_command_live_output(
                                    &state,
                                    cmd,
                                    "Server".into(),
                                    Some(tx),
                                    Some(stdin_clone),
                                )
                                .await
                                .ok();
                            });
                            let _ = cmd_tx.send("Server started".into()).await;
                        } 
                        Ok(())
                    } else {
                        Ok(())
                    }
                } else {
                    Ok(())
                }
            }
            "set_server" => {
                
                Ok(())
            }
            _ => {
                Ok(())
            }
        }
    } else {
        //Ok(())
        Err(Error::new(
            ErrorKind::Other,
            "This should not be here"
        ))?
    }
}

// Gets a provider out of a handpicked list of gameservers, including custom, at some point needs to be massively re-worked as
// it might be a bit messy having this is my rust code, the majority of the code and types are in provider.rs and it just relies on
// structs I created changing into this provider types, which is one of the few reasons why a better system is needed, it also takes a path to put the files in (not implimented yet)
fn get_provider_object(option_name: Option<&str>, option_path: Option<&str>) -> Option<ProviderType> {
    if option_name.is_none() || option_path.is_none() {
        return None
    }
    let (path, name) = (option_path.unwrap(), option_name.unwrap());

    let mut path = path.to_string();
    if !path.starts_with("server/") {
        path = format!("server/{}", path);
        println!("Adjusted path to: {}", path);
    }

    match name {
        "minecraft" => {
            println!("Selected provider: Minecraft");
            Some(Minecraft.into())
        }
        "custom" => {
            let provider_json_path = format!("{}/provider.json", path);
            println!(
                "Looking for custom provider config at: {}",
                provider_json_path
            );

            match std::fs::read_to_string(&provider_json_path) {
                Ok(json_content) => {
                    println!("Successfully read provider.json");
                    match serde_json::from_str::<ProviderConfig>(&json_content) {
                        Ok(config) => {
                            println!("Loaded custom provider config successfully");
                            let mut custom = Custom::new();

                            if let Some(cmd) = config.pre_hook {
                                println!("Adding pre_hook: {}", cmd);
                                custom = custom.with_pre_hook(cmd);
                            }
                            if let Some(cmd) = config.install {
                                println!("Adding install: {}", cmd);
                                custom = custom.with_install(cmd);
                            }
                            if let Some(cmd) = config.post_hook {
                                println!("Adding post_hook: {}", cmd);
                                custom = custom.with_post_hook(cmd);
                            }
                            if let Some(cmd) = config.start {
                                println!("Adding start: {}", cmd);
                                custom = custom.with_start(cmd);
                            }

                            Some(custom.into())
                        }
                        Err(e) => {
                            println!("[ERROR] Failed to parse provider.json: {}", e);
                            Some(Custom::new().into())
                        }
                    }
                }
                Err(e) => {
                    println!(
                        "[WARN] Could not read provider.json at {}: {}",
                        provider_json_path, e
                    );
                    None
                }
            }
        }
        _ => {
            println!("[WARN] Unknown provider: {}", name);
            None
        }
    }
}

// Needs to be implimented, provider and servername should not forever remain the same thing and
// a index needs to be kept about what provider matches to which server, the code already store the data
// but at some point i need to decouple the two
async fn get_provider_from_servername(state: &AppState, name: Option<String>) -> Option<String> {
    if name.is_some() {
        let server_path = state.server_index.iter().find(|(server_name, _)| name.clone().unwrap() == **server_name);
        if server_path.is_some() {
            if let Some((_, server_index)) = server_path {
                Some(server_index.provider.clone())
            } else {
                None
            }
        } else {
            None
        }
    } else {
        None
    }
}

// paths are tagged (with the name), this is for nested servers within the server directory, so you can have the files of multiple servers in one node, and the string returned is added to the path
// that create server or anything about the server before the process is created or after the process finishes needs to know
async fn get_definite_path_from_name(state: &AppState, name: Option<String>) -> Option<String> {
    if name.is_some() {
        let server_path = state.server_index.iter().find(|(server_name, _)| name.clone().unwrap() == **server_name);
        if server_path.is_some() {
            if let Some((_, server_index)) = server_path {
                Some(server_index.location.clone())
            } else {
                None
            }
        } else {
            None
        }
    } else {
        None
    }
}
