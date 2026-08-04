use chrono::format::Item;
use chrono::DateTime;
use chrono::Local;
use futures::stream::unfold;
use futures::Stream;
use futures::StreamExt;
use general_networked_filesystem::FileRequest;
use general_networked_filesystem::FileRequestExecutable;
use network_abstraction_lib::any_type;
use network_abstraction_lib::erase;
use network_abstraction_lib::erase_stream_wrapper_result;
use network_abstraction_lib::ErrorResponse;
use network_abstraction_lib::ExtractResponse;
use network_abstraction_lib::ExtractorErrors;
use network_abstraction_lib::HandlerType;
use network_abstraction_lib::IntoRequest;
use network_abstraction_lib::IntoResponse;
use network_abstraction_lib::erase_string_wrapper;
use network_abstraction_lib::erase_stream_wrapper;
use network_abstraction_lib::MiddlewareAction;
use network_abstraction_lib::NoneResponse;
use network_abstraction_lib::StreamResponse;
use network_abstraction_lib::ValueRequest;
use serde_json::{json, Value};
use tokio::sync::broadcast::Receiver;
use std::convert::TryFrom;
use std::fmt;
use std::io::Error;
use std::io::ErrorKind;
use std::path::Path;
use std::pin::Pin;
use std::process::{Command, Stdio};
use std::sync::Arc;
use tokio::fs;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpListener;
use tokio::process::Child;
use tokio::process::{ChildStdin, Command as TokioCommand};
use tokio::sync::{mpsc, Mutex};

use crate::broadcast::Sender;
use crate::databasespec::Filters;
use crate::databasespec::ServerMetadata;
// use crate::filesystem::cleanup_end_file_markers;
// use crate::filesystem::execute_file_operation;
// use crate::filesystem::get_files_content;
// use crate::filesystem::get_metadata;
// use crate::filesystem::list_directory;
// use crate::filesystem::list_directory_with_range;
// use crate::filesystem::send_folder_over_broadcast;
// use crate::filesystem::BasicPath;
// use crate::filesystem::FileChunk;
// use crate::filesystem::FileOperations;
use crate::providers::{Custom, Platforms, Provider, ProviderConfig, ProviderDbList, ProviderGame};
use crate::transport::node_transport::ConnectionHandler;
use crate::transport::node_transport::ConnectionManager;
use crate::transport::node_transport::ConsoleRequest;
use crate::transport::node_transport::CreateServerRequest;
use crate::transport::node_transport::DeleteServerRequest;
use crate::transport::node_transport::FileOperationResponse;
// use crate::transport::node_transport::NodeTransportable;
use crate::transport::node_transport::Ping;
use crate::transport::node_transport::PingResponse;
use crate::transport::node_transport::ServerDataRequest;
use crate::transport::node_transport::ServerDataResponse;
use crate::transport::node_transport::ServerNameRequest;
use crate::transport::node_transport::ServerNameResponse;
use crate::transport::node_transport::ServerStateRequest;
use crate::transport::node_transport::ServerStateResponse;
use crate::transport::node_transport::SetFilterRequest;
use crate::transport::node_transport::SetServerRequest;
use crate::transport::node_transport::StartServerRequest;
use crate::transport::node_transport::StopServerRequest;
use crate::transport::node_transport::TryIntoRequest;
use tokio::net::TcpStream;

use std::net::SocketAddr;
use tokio::sync::broadcast;

use network_abstraction_lib::Router;

// I use the same code as in the main server
// with a few diffrences in stuff like filesystem
mod databasespec;
mod extra;
mod intergrations;
mod jsondatabase;
mod providers;

mod transport;

use databasespec::ServerIndex;

#[cfg(feature = "postgres")]
mod database {
    include!("pgdatabase.rs");
}
#[cfg(feature = "postgres")]
use pgdatabase::{load_db, save_db, DbConn};

#[cfg(not(feature = "postgres"))]
mod database {
    include!("jsondatabase.rs");
}
#[cfg(not(feature = "postgres"))]
use jsondatabase::{load_db, save_db, DbConn};

// use jsondatabase::{load_db, save_db};

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

// TODO: consider this
// IncomingMessageWithMetadata has been omitted here but rather imported as
// some connection types require it to the the same as some internal defintion of
// Incoming

// newer version of IncomingMessage, mainly because this includes the metadata feild which i sometimes use
// IncomingMessageWithMetadata and IncomingMessage should be renamed to something that makes sense
// Note, this also handles the things like MessagePayloadWithMetadata and converts it here, as
// from the gameservers perpective, the command payload is incoming, so it made sense not to recreate such a struct
#[derive(Debug, serde::Serialize, serde::Deserialize, Clone)]
struct IncomingMessageWithMetadata {
    message: String,
    #[serde(rename = "type")]
    message_type: String,
    metadata: MetadataTypes,
    authcode: String,
}

// For very simple messages like pings that need no added complexity
#[derive(Debug, serde::Serialize, serde::Deserialize, Clone, Default)]
struct SimpleMessage {
    message: String,
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
        sandbox: bool,
        server_metadata: ServerMetadata,
    },
    Filter(Filters),
    DeleteServer {
        delete_server_name: String,
        delete_server_files: bool,
    },
    //DeleteServerFiles(bool),
    // TODO: replace this with EXPLICIT metadata types per action
    Boolean(bool),
    String(String),
}

// This is used in convert_provider,
// an abstraction for now, that manages conversions between
// paths, names, objects, sandbox, etc
// you specify an order of operations, and an expected return type
// if it can find what was asked from some operation (starting from left to right because its a vector/array)
// it will return it, otherwise it will continue onto the next bit of information until it can return the request peice of data
// ProviderTypes is whats inputted and outputted,
// ProviderReturnTypes is what you expect to be returned (does not need an argument)
#[derive(Debug)]
enum ProviderTypes {
    Path(String),
    Object((String, Platforms)),
    Name(String),
    Sandbox(bool),
    Provider(String),
}

#[derive(Debug)]
enum ProviderReturnTypes {
    Path,
    Object,
    Name,
    Sandbox,
    Provider,
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
// TODO: phase out, this works on the outdated model where i send everything in one route which is automatically proxied
// to the node, but this should not always be the case, atleast this was the primary use
#[derive(Debug, serde::Deserialize, serde::Serialize, Clone)]
#[serde(tag = "kind", content = "data")]
enum ApiCalls {
    None,
    Capabilities(Vec<String>),
    NodeList(Vec<String>),
    IncomingMessage(MessagePayload),
    Node(Node),
    // FileOperations(FileOperations),
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

// #[derive(Default)]
// struct NoneResponse {}
// impl IntoResponse for NoneResponse {}

// These ip:port defaults are diffrence based on feature as i typically do not run full-stack
// when im testing on bare metal, where the ip:port have to be diffrent to not conflict
#[cfg(feature = "full-stack")]
static StaticLocalUrl: &str = "0.0.0.0:8080";

#[cfg(not(feature = "full-stack"))]
static StaticLocalUrl: &str = "0.0.0.0:8082";

// the server state, currently only holds keywords for what messages to look for when declaring the server as started or stopped
// might be phased out in favor of determining whether or not the process is running or not
#[derive(serde::Serialize)]
struct GetState {
    name: String,
    start_keyword: String,
    stop_keyword: String,
}

// At the moment of writing this, some form of filter is required, because if too many logs
// are sent, then the data pipeline halts
// secondly, remove the need for a filter entirely by keeping a message queue with throttling
fn filter(filter: Filters, line: String) -> bool {
    // let mut db = state.db.lock().await;
    // db.filter = filter.clone();
    // save_db(&db);
    match filter {
        Filters::AlternatingLine => {
            if line.contains('%') {
                if let Some(pct_str) = line
                    .split('%')
                    .next()
                    .and_then(|s| s.split_whitespace().last())
                    .and_then(|s| s.parse::<u32>().ok())
                {
                    if pct_str % 5 != 0 {
                        true
                    } else {
                        false
                    }
                } else {
                    true
                }
            } else {
                false
            }
        }
        Filters::None => false,
    }
}

// For any time a server related process will run, this process hook needs to run before that
// if the user enabled the sandbox for a server, or they were forced too by lack of permissions
// (e.g a admin set sandbox to always enable), then this will use bwrap to wrap the command
// and only expose needed binaries, note for this reason, if an admin did not give a user
// permission to disable the sandbox, they should also have no control over the paths and commands to be
// in the sandbox too.
// Bwrap adds a system dep, but is more tested and simpler to use
fn process_hook(
    state: &AppState,
    provider: ProviderConfig,
    sandbox: bool,
    location_option: Option<String>,
    cmd: &mut TokioCommand,
) {
    let uid = unsafe { libc::getuid() };
    let sandbox_override: bool = get_env_var_or_arg("DISABLE_SANDBOX", Some(false)).unwrap();
    if sandbox && uid == 0 && !sandbox_override {
        #[cfg(target_os = "linux")]
        {
            let cwd = std::env::current_dir().unwrap_or_default();
            let location = location_option.unwrap_or_default();
            let location_stripped = location.trim_start_matches("server/");
            let resolved = cwd.join("server").join(location_stripped);
            let resolved_str = resolved.to_string_lossy().trim_end_matches('/').to_string();

            let _ = std::fs::create_dir_all(&resolved_str);

            let bwrap_path = Command::new("which")
                .arg("bwrap")
                .output()
                .ok()
                .filter(|o| o.status.success())
                .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
                .unwrap_or_else(|| "bwrap".to_string());

            let mut bwrap_args: Vec<String> = vec![
                "--bind".into(),
                resolved_str.clone(),
                "/server".into(),
                "--ro-bind-try".into(),
                "/nix".into(),
                "/nix".into(),
                "--ro-bind-try".into(),
                "/run".into(),
                "/run".into(),
                "--ro-bind-try".into(),
                "/lib".into(),
                "/lib".into(),
                "--ro-bind-try".into(),
                "/lib64".into(),
                "/lib64".into(),
                "--ro-bind-try".into(),
                "/usr/lib".into(),
                "/usr/lib".into(),
                "--ro-bind-try".into(),
                "/usr/lib64".into(),
                "/usr/lib64".into(),
                "--ro-bind-try".into(),
                "/etc".into(),
                "/etc".into(),
                "--proc".into(),
                "/proc".into(),
                "--dev".into(),
                "/dev".into(),
                "--tmpfs".into(),
                "/tmp".into(),
                "--chdir".into(),
                "/server".into(),
                "--share-net".into(),
                "--unshare-ipc".into(),
                "--unshare-pid".into(),
                "--unshare-uts".into(),
                "--setenv".into(),
                "PATH".into(),
                "/run/current-system/sw/bin:/usr/local/bin:/usr/bin:/bin".into(),
            ];

            let mut all_needed_commands = provider.needed_commands;
            all_needed_commands.push("sh".to_string());
            for command in all_needed_commands {
                let command_path = Command::new("which")
                    .arg(&command)
                    .output()
                    .ok()
                    .filter(|o| o.status.success())
                    .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
                    .unwrap_or_else(|| format!("/run/current-system/sw/bin/{}", command));

                let command_real = std::fs::canonicalize(&command_path)
                    .unwrap_or_else(|_| std::path::PathBuf::from(&command_path));
                let command_real_str = command_real.to_string_lossy().to_string();
                bwrap_args.push("--ro-bind-try".into());
                bwrap_args.push(command_real_str.clone());
                bwrap_args.push(command_real_str);
            }
            for path in provider.needed_paths {
                bwrap_args.push("--ro-bind-try".into());
                bwrap_args.push(path.clone());
                bwrap_args.push(path);
            }

            let current_program = cmd.as_std().get_program().to_string_lossy().to_string();
            let current_args: Vec<String> = cmd
                .as_std()
                .get_args()
                .map(|a| a.to_string_lossy().to_string())
                .map(|a| {
                    if a.starts_with("cd ") {
                        if let Some(rest) = a.splitn(2, " && ").nth(1) {
                            return rest.to_string();
                        }
                    }
                    a
                })
                .collect();

            println!(
                "sandbox: wrapping command: {} {:?}",
                current_program, current_args
            );

            *cmd = TokioCommand::new(&bwrap_path);
            for arg in &bwrap_args {
                cmd.arg(arg);
            }
            cmd.arg(current_program);
            for arg in current_args {
                cmd.arg(arg);
            }
            cmd.stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped());

            println!("sandbox: bwrap command set up");
        }
        #[cfg(not(target_os = "linux"))]
        {
            println!("Skipping process jailing on non-linux");
        }
    }
}
// runs a command and forwards the output of the command to the given channel, which in this case would be back to
// the main server
async fn run_command_live_output(
    state: &AppState,
    cmd: Command,
    sandbox: bool,
    location: String,
    provider: ProviderConfig,
    label: String,
    sender: Option<mpsc::Sender<String>>,
    stdin_arc: Option<Arc<Mutex<Option<ChildStdin>>>>,
    timeout: Option<u64>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let db = state.db.lock().await;
    let current_filter = db.filter.clone();
    save_db(&db);
    drop(db);

    let mut tokio_cmd = TokioCommand::from(cmd);
    let cwd = std::env::current_dir().unwrap_or_default();
    let location_stripped = location.trim_start_matches("server/");
    let resolved = cwd.join("server").join(location_stripped);
    let resolved_str = resolved.to_string_lossy().trim_end_matches('/').to_string();

    tokio_cmd
        .current_dir(&resolved_str)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .stdin(Stdio::piped());
    println!("{:#?}", sandbox.clone());
    println!("Befre process hook");
    process_hook(state, provider, sandbox, Some(location), &mut tokio_cmd);
    println!("After process hook");
    let mut child = tokio_cmd.spawn()?;

    if let Some(stdin_slot) = stdin_arc {
        let child_stdin = child.stdin.take();
        *stdin_slot.lock().await = child_stdin;
    }

    let current_filter_clone = current_filter.clone();
    let stdout_last_updated_clone = state.last_updated.clone();
    let stdout_handle = if let Some(stdout) = child.stdout.take() {
        let tx = sender.clone();
        let lbl = label.clone();
        Some(tokio::spawn(async move {
            let mut reader = BufReader::new(stdout).lines();
            let stdout_last_updated_loop_clone = stdout_last_updated_clone.clone();
            while let Ok(Some(line)) = reader.next_line().await {
                let mut stdout_last_updated_clone_guard =
                    stdout_last_updated_loop_clone.lock().await;
                *stdout_last_updated_clone_guard = Some(Local::now());
                if filter(current_filter_clone.clone(), line.clone()) {
                    continue;
                }
                if let Some(tx) = &tx {
                    let msg =
                        json!({"type":"stdout","data":format!("[{}] {}", lbl, line)}).to_string();
                    let _ = tx.try_send(msg);
                }
            }
        }))
    } else {
        None
    };

    let stderr_last_updated_clone = state.last_updated.clone();
    let stderr_handle = if let Some(stderr) = child.stderr.take() {
        let tx = sender.clone();
        let lbl = label.clone();
        Some(tokio::spawn(async move {
            let mut reader = BufReader::new(stderr).lines();
            let stderr_last_updated_loop_clone = stderr_last_updated_clone.clone();
            while let Ok(Some(line)) = reader.next_line().await {
                let mut stderr_last_updated_clone = stderr_last_updated_loop_clone.lock().await;
                *stderr_last_updated_clone = Some(Local::now());
                if filter(current_filter.clone(), line.clone()) {
                    continue;
                }
                if let Some(tx) = &tx {
                    let msg =
                        json!({"type":"stderr","data":format!("[{}] {}", lbl, line)}).to_string();
                    let _ = tx.try_send(msg);
                }
            }
        }))
    } else {
        None
    };

    if let Some(timeout_number) = timeout {
        match tokio::time::timeout(
            tokio::time::Duration::from_millis(timeout_number),
            child.wait(),
        )
        .await
        {
            Err(_) => {
                println!("Command timed out, killing process");
                let _ = child.kill().await;
                return Err(Box::new(Error::new(
                    ErrorKind::TimedOut,
                    "Command timed out",
                )));
            }
            Ok(wait_result) => {
                if let Err(e) = wait_result {
                    println!("Error waiting for process: {}", e);
                    return Err(e.into());
                }
            }
        }
    } else {
        child.wait().await?;
    }

    println!("Post-hook process exited");
    // TODO: have it tell the main server that it exited
    if let Some(h) = stdout_handle {
        let _ = h.await;
    }
    if let Some(h) = stderr_handle {
        let _ = h.await;
    }
    println!("Post-hook output fully flushed");

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
// #[derive(serde::Serialize, serde::Deserialize, Debug)]
// struct FileRequestMessage {
//     id: u64,
//     #[serde(flatten)]
//     payload: FileRequestPayload,
// }

// The types of file requests the server can make, easy to match and keep track of/consistent
// #[derive(serde::Serialize, serde::Deserialize, Debug)]
// #[serde(tag = "type", content = "data")]
// enum FileRequestPayload {
//     Metadata {
//         path: String,
//     },
//     ListDir {
//         path: String,
//     },
//     ListDirWithRange {
//         path: String,
//         start: Option<u64>,
//         end: Option<u64>,
//     },
//     PathFromTag {
//         path: String,
//         tag: Option<String>,
//     },
//     FileChunk(FileChunk),
// }

// Needs to be phased out, or just removed, everything now uses FileRequestPayload
// #[derive(serde::Serialize, serde::Deserialize)]
// struct FileResponseMessage {
//     in_response_to: u64,
//     data: serde_json::Value,
// }

// AppState for the Node, stores the name of the current server, the state of the process
// whether or not its running, the channel for the output messages, and the process, makes it easier to pass and modify
// between functions
struct AppState {
    // current_server has to be an arc mutex because you cant assign data to an arc
    current_server: Arc<Mutex<Option<String>>>,
    //server_index: HashMap<String, ServerIndex>,
    jailed_user: String,
    authenticated_origins: Arc<Mutex<Vec<String>>>,
    server_running: Arc<Mutex<bool>>,
    output_tx: Arc<Mutex<Option<mpsc::Sender<String>>>>,
    cmd_rx: Mutex<Option<mpsc::Receiver<String>>>,
    cmd_tx: Mutex<Option<Arc<mpsc::Sender<String>>>>,
    stdin_ref: Arc<Mutex<Option<ChildStdin>>>,
    server_output_tx: Arc<Mutex<Option<broadcast::Sender<String>>>>,
    server_process: Arc<Mutex<Option<Child>>>,
    last_updated: Arc<Mutex<Option<DateTime<Local>>>>,
    // Consider if i want to store the db at all, previously I was wondering whether or not to have an arc mutex, (arc not needed; the app state has a arc, so I just need to add a mutex
    // so now any changes will still be in sync so you never have a case of a longer operation based on older data writting to the db overwriting the newer one).
    // now I am considering if I need db at all, ill keep it here for now to consider parity with the main gameserver node based on design choices.
    db: Arc<Mutex<databasespec::Database>>,
    db_conn: Arc<Mutex<Option<DbConn>>>,
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
    let (tx, rx) = broadcast::channel::<Vec<u8>>(16);

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

// Looks for a env varible, if its not found, try the specified default, if none is found it will use the default of whatever that type is
fn get_env_var_or_arg<T: std::str::FromStr>(env_var: &str, default: Option<T>) -> Option<T> {
    env::var(env_var)
        .ok()
        .and_then(|s| s.parse().ok())
        .or(default)
}

async fn create_server_handler(state: Arc<AppState>, req: CreateServerRequest) -> Result<StreamResponse<String>, ErrorResponse> {
    let cmd_tx_arc = state.cmd_tx.lock().await.clone().unwrap();
    let cmd_tx = cmd_tx_arc;
    let stdin_ref = state.stdin_ref.clone();
    let _ = create_server(
        state.clone(),
        &cmd_tx,
        &stdin_ref,
        serde_json::to_value(req.clone()).unwrap(),
    )
    .await;
    println!("Finished creating server");
    //NoneResponse {}
    if let Some(rx) = state.cmd_rx.lock().await.take() {
        let stream = unfold(rx, |mut rx| async {
            match rx.recv().await {
                Some(value) => Some((value, rx)),
                None => None,
            }
        });
        Ok(StreamResponse::new(stream))
    } else {
        Err(ErrorResponse { error: "stream taken".to_string() })
    }
} 
async fn start_server_handler(state: Arc<AppState>, req: StartServerRequest) -> Result<StreamResponse<String>, ErrorResponse> {
    //let current_server = state.current_server.lock().await;
    let stdin_ref = &state.stdin_ref;
    let cmd_tx_arc = state.cmd_tx.lock().await.clone().unwrap();
    let cmd_tx = cmd_tx_arc;
    {
        let stdin_guard = stdin_ref.lock().await;
        if stdin_guard.is_some() {
            let _ = cmd_tx
                .send("Server is already running. Use 'stop_server' first.".into())
                .await;
        }
    }
    if let Some(current_server) = state.current_server.lock().await.clone() {
        // let provider =
        //     get_provider_from_servername(&state, Some(current_server.clone())).await;
        let location = {
            if let Some(ProviderTypes::Path(path)) = convert_provider(
                state.clone(),
                vec![ProviderTypes::Name(current_server.clone())],
                ProviderReturnTypes::Path,
            )
            .await
            {
                Some(path)
            } else {
                None
            }
        };
        // println!("DEBUG current_server: '{}'", current_server);
        // println!("DEBUG location: '{:?}'", location);
        let provider = {
            if let Some(ProviderTypes::Provider(provider)) = convert_provider(
                state.clone(),
                vec![ProviderTypes::Name(current_server.clone())],
                ProviderReturnTypes::Provider,
            )
            .await
            {
                Some(provider)
            } else {
                None
            }
        };
        let provider_object = {
            if let Some(ProviderTypes::Object(object)) = convert_provider(
                state.clone(),
                vec![
                    ProviderTypes::Path(location.clone().unwrap_or(String::new())),
                    ProviderTypes::Provider(provider.unwrap_or(String::new())),
                ],
                ProviderReturnTypes::Object,
            )
            .await
            {
                Some(object)
            } else {
                None
            }
        };
        if let Some((_, provider_platform)) = provider_object {
            let provider_game_option = pick_platform(provider_platform).map(|config| {
                let provider_game: ProviderGame = config.into();
                provider_game
            });
            if let Some(mut provider_game_commands) = provider_game_option {
                if let Some(ref loc) = location {
                    let _ = provider_game_commands.set_location(loc.to_owned());
                }
                if let Some(cmd) = provider_game_commands.start() {
                    let tx = cmd_tx.clone();
                    let stdin_clone = stdin_ref.clone();

                    let sandbox = {
                        if let Some(ProviderTypes::Sandbox(sandbox)) = convert_provider(
                            state.clone(),
                            vec![ProviderTypes::Name(current_server.clone())],
                            ProviderReturnTypes::Sandbox,
                        )
                        .await
                        {
                            sandbox
                        } else {
                            false
                        }
                    };
                    let location = {
                        if let Some(ProviderTypes::Path(path)) = convert_provider(
                            state.clone(),
                            vec![ProviderTypes::Name(current_server.clone())],
                            ProviderReturnTypes::Path,
                        )
                        .await
                        {
                            Some(path)
                        } else {
                            None
                        }
                    };
                    let provider = {
                        if let Some(ProviderTypes::Provider(provider)) = convert_provider(
                            state.clone(),
                            vec![ProviderTypes::Name(current_server.clone())],
                            ProviderReturnTypes::Provider,
                        )
                        .await
                        {
                            Some(provider)
                        } else {
                            None
                        }
                    };
                    let provider_object = {
                        if let Some(ProviderTypes::Object(object)) = convert_provider(
                            state.clone(),
                            vec![
                                ProviderTypes::Path(location.clone().unwrap_or(String::new())),
                                ProviderTypes::Provider(provider.unwrap_or(String::new())),
                            ],
                            ProviderReturnTypes::Object,
                        )
                        .await
                        {
                            Some(object)
                        } else {
                            None
                        }
                    };

                    let platform = provider_object
                        .unwrap_or(("".to_string(), Platforms::default()))
                        .1;
                    let provider = pick_platform(platform).unwrap_or(ProviderConfig::default());
                    let arc_state_for_stdin = state.clone();
                    tokio::spawn(async move {
                        let result = run_command_live_output(
                            &arc_state_for_stdin,
                            cmd,
                            sandbox,
                            location.unwrap_or(String::new()),
                            provider,
                            "Server".into(),
                            Some((*tx).clone()),
                            Some(stdin_clone.clone()),
                            None,
                        )
                        .await;
                        {
                            let mut stdin_guard = stdin_clone.lock().await;
                            *stdin_guard = None;
                        }
                        match result {
                            Ok(_) => {
                                let _ = tx.send("Server process ended".into()).await;
                                let (cmd_tx, cmd_rx) = mpsc::channel::<String>(10_000);
                                *arc_state_for_stdin.cmd_tx.lock().await = Some(Arc::new(cmd_tx));
                                *arc_state_for_stdin.cmd_rx.lock().await = Some(cmd_rx);
                            }
                            Err(e) => {
                                let _ = tx.send(format!("Server process failed: {}", e)).await;
                            }
                        }
                    });

                    let _ = cmd_tx.send("Server started".into()).await;
                } else {
                    let _ = cmd_tx
                        .send("No start command available for this provider".into())
                        .await;
                }
            } else {
                // let _ = cmd_tx
                //     .send("Failed to get provider for server".into())
                //     .await;
            }
        }
    }
    if let Some(rx) = state.cmd_rx.lock().await.take() {
        let stream = unfold(rx, |mut rx| async {
            match rx.recv().await {
                Some(value) => Some((value, rx)),
                None => None,
            }
        });
        Ok(StreamResponse::new(stream))
    } else {
        println!("C");
        Err(ErrorResponse { error: "stream taken".to_string() })
    }
    //NoneResponse {}
}
async fn stop_server_handler(
    state: Arc<AppState>,
    req: StopServerRequest,
) -> Result<NoneResponse, ErrorResponse> {
    let stdin_ref = state.stdin_ref.clone();
    if let Some(cmd_tx_arc) = state.cmd_tx.lock().await.clone() {
        let cmd_tx = cmd_tx_arc;
        println!("Got a stop server request");
        if let Some(current_server) = state.current_server.lock().await.clone() {
            let option_path = {
                if let Some(ProviderTypes::Path(path)) = convert_provider(
                    state.clone(),
                    vec![ProviderTypes::Name(current_server.clone())],
                    ProviderReturnTypes::Path,
                )
                .await
                {
                    Some(path)
                } else {
                    None
                }
            };
            let provider = {
                if let Some(ProviderTypes::Provider(provider)) = convert_provider(
                    state.clone(),
                    vec![ProviderTypes::Name(current_server.clone())],
                    ProviderReturnTypes::Provider,
                )
                .await
                {
                    Some(provider)
                } else {
                    None
                }
            };
            let provider_object = {
                if let Some(ProviderTypes::Object(object)) = convert_provider(
                    state.clone(),
                    vec![
                        ProviderTypes::Path(option_path.unwrap_or(String::new())),
                        ProviderTypes::Provider(provider.unwrap_or(String::new())),
                    ],
                    ProviderReturnTypes::Object,
                )
                .await
                {
                    Some(object)
                } else {
                    None
                }
            };

            if let Some(_) = provider_object {
                let input = "stop";
                let mut guard = stdin_ref.lock().await;
                if let Some(stdin) = guard.as_mut() {
                    let _ = stdin.write_all(format!("{}\n", input).as_bytes()).await;
                    let _ = stdin.flush().await;
                    let _ = cmd_tx.send(format!("Sent to server: {}", input)).await;
                }
            }
        }

        Ok(NoneResponse {})
    } else {
        Err(ErrorResponse { error: "could not stop server".to_string() })
    }
}
async fn delete_server_handler(state: Arc<AppState>, req: DeleteServerRequest) -> NoneResponse {
    if let MetadataTypes::DeleteServer {
        delete_server_name,
        delete_server_files,
    } = req.common.metadata
    {
        let option_path = {
            if let Some(ProviderTypes::Path(path)) = convert_provider(
                state.clone(),
                vec![ProviderTypes::Name(delete_server_name.clone())],
                ProviderReturnTypes::Path,
            )
            .await
            {
                Some(path)
            } else {
                None
            }
        };
        if delete_server_files {
            if let Some(mut path) = option_path {
                if !path.trim().starts_with("server") && !path.trim().starts_with("server/") {
                    path = format!("server/{}", path);
                }
                if let Err(errro) = fs::remove_dir_all(&path).await {
                    eprintln!("Failed to delete directory {}: {}", path, errro);
                }
            }
        }
        if let Some(current_server) = state.clone().current_server.lock().await.clone() {
            //let inner_cloned_db_mutex = cloned_db_mutex.clone();
            let mut db = state.db.lock().await;
            if *delete_server_name == current_server {
                *(state.current_server.clone()).lock().await = None;
                db.current_server = String::new();
            }
            db.server_index.remove(&delete_server_name);
            save_db(&db);
            drop(db);
        }
    }
    NoneResponse {}
}
async fn set_server_handler(state: Arc<AppState>, req: SetServerRequest) -> NoneResponse {
    println!("Got a set server request");
    if let MetadataTypes::Server {
        servername,
        provider,
        location,
        providertype,
        sandbox,
        server_metadata,
    } = req.common.metadata
    {
        let mut db = state.db.lock().await;
        db.current_server = servername.clone();
        // Ensures the current info is up to date in the server index
        db.server_index
            .entry(servername.clone())
            .or_insert_with(|| ServerIndex {
                location: if location.is_empty() {
                    format!("server/{}", servername)
                } else if location.starts_with("server/") {
                    location.clone()
                } else {
                    format!("server/{}", location)
                },
                provider: provider.clone(),
                providertype: providertype.clone(),
                sandbox,
                server_metadata: server_metadata.clone(),
            });
        save_db(&db);
        let mut mutable_server = state.current_server.lock().await;
        *mutable_server = Some(servername.to_string());
    }
    NoneResponse {}
}
async fn set_filter_handler(state: Arc<AppState>, req: SetFilterRequest) -> NoneResponse {
    if let MetadataTypes::Filter(filter) = req.common.metadata {
        let mut db = state.db.lock().await;
        db.filter = filter.clone();
        save_db(&db);
    }
    NoneResponse {}
}
async fn console_handler(state: Arc<AppState>, req: ConsoleRequest) -> NoneResponse {
    println!("Got a console request");
    let input = req.common.message.clone();
    let stdin_ref = &state.stdin_ref;
    let cmd_tx_arc = state.cmd_tx.lock().await.clone().unwrap();
    let cmd_tx = cmd_tx_arc;
    let mut guard = stdin_ref.lock().await;
    if let Some(stdin) = guard.as_mut() {
        let _ = stdin.write_all(format!("{}\n", input).as_bytes()).await;
        let _ = stdin.flush().await;
        let _ = cmd_tx.send(format!("Sent to server: {}", input)).await;
    }
    NoneResponse {}
}
async fn server_data_handler(
    state: Arc<AppState>,
    req: ServerDataRequest,
) -> Result<ServerDataResponse, NoneResponse> {
    println!("Got a server data request");
    if let Some(current_server) = state.current_server.lock().await.clone() {
        let option_path = {
            if let Some(ProviderTypes::Path(path)) = convert_provider(
                state.clone(),
                vec![ProviderTypes::Name(current_server.clone())],
                ProviderReturnTypes::Path,
            )
            .await
            {
                Some(path)
            } else {
                None
            }
        };
        let provider = {
            if let Some(ProviderTypes::Provider(provider)) = convert_provider(
                state.clone(),
                vec![ProviderTypes::Name(current_server.clone())],
                ProviderReturnTypes::Provider,
            )
            .await
            {
                Some(provider)
            } else {
                None
            }
        };
        let provider_object = {
            if let Some(ProviderTypes::Object(object)) = convert_provider(
                state.clone(),
                vec![
                    ProviderTypes::Path(option_path.unwrap_or(String::new())),
                    ProviderTypes::Provider(provider.unwrap_or(String::new())),
                ],
                ProviderReturnTypes::Object,
            )
            .await
            {
                Some(object)
            } else {
                None
            }
        };

        if let Some((_, provider)) = provider_object {
            if let Some(platform) = pick_platform(provider) {
                println!("Sending out the info");
                let server_data_response = ServerDataResponse {
                    state: GetState {
                        name: platform.default_name.unwrap_or("".to_string()),
                        start_keyword: platform.start_keyword.unwrap_or("".to_string()),
                        stop_keyword: platform.stop_keyword.unwrap_or("".to_string()),
                    },
                };
                Ok(server_data_response)
            } else {
                Err(NoneResponse {})
            }
            //server_data_response
        } else {
            Err(NoneResponse {})
        }
    } else {
        Err(NoneResponse {})
    }
    //NoneResponse {}
}
async fn ping_handler(state: Arc<AppState>, req: Ping) -> PingResponse {
    println!("got ping request");
    //         //let out_tx_clone = out_tx.clone();
    let pong = PingResponse {
        message: SimpleMessage {
            message: "pong".to_string(),
        },
    };
    pong
}
async fn server_state_handler(
    state: Arc<AppState>,
    req: ServerStateRequest,
) -> ServerStateResponse {
    //println!("Got a server state request");
    let status = &state.server_running.lock().await;
    //println!("{:#?}", status);
    let server_state_response = ServerStateResponse {
        message: MessagePayload {
            r#type: "server_state".to_string(),
            message: status.to_string(),
            authcode: "0".to_string(),
        },
    };
    server_state_response
}
async fn server_name_handler(state: Arc<AppState>, req: ServerNameRequest) -> ServerNameResponse {
    println!("Got a server name request");
    // let hostname_str = match hostname_ref.clone() {
    //     Ok(os) => os.to_string_lossy().to_string(),
    //     Err(e) => e.clone(),
    // };
    let hostname = hostname::get().unwrap_or("unknown".into());
    let server_name_response = ServerNameResponse {
        message: MessagePayload {
            r#type: "command".to_string(),
            message: hostname.into_string().unwrap(),
            authcode: "0".to_string(),
        },
    };
    server_name_response
}

async fn check_server(arc_state: Arc<AppState>, server_output_rx: &mut Option<Receiver<String>>, needs_server_status_check: &mut bool) -> Option<String> {
    if *needs_server_status_check {
        let server_running_lock = arc_state.server_running.lock().await;
        let output_tx_lock = arc_state.server_output_tx.lock().await;

        if *server_running_lock {
            if let Some(ref tx) = *output_tx_lock {
                *server_output_rx = Some(tx.subscribe());
                *needs_server_status_check = false;

                let connection_msg = serde_json::json!({
                    "type": "info",
                    "data": "Connected to server output stream",
                    "authcode": "0"
                })
                .to_string()
                    + "\n";

                Some(connection_msg.to_string())
                // if let Err(e) = out_tx.send(connection_msg.to_string()).await {
                //     eprintln!("[{}] Write error: {}", addr_clone, e);
                //     break;
                // };
                // }
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

async fn spawn_request_loop(
    //arc_state: Arc<AppState>,
    conn_handler: &mut ConnectionHandler,
    router: &mut Router<Arc<AppState>>,
    //cmd_rx: &mut mpsc::Receiver<String>, 
    addr: String
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let kill_socket = false;
    println!("[{}] DEBUG: Connection task started", addr);

    //let (mut read_half, mut write_half) = socket.into_split();

    // let mut output_tx = arc_state_clone.output_tx.lock().await;
    // *output_tx = Some(out_tx.clone());
    let arc_state = router.get_state();
    let (out_tx, mut out_rx) = mpsc::channel::<String>(128);
    //let inner_out_tx = out_tx.clone();
    if let Ok(mut guard) = arc_state.output_tx.try_lock() {
        println!("assigning out_tx");
        *guard = Some(out_tx.clone());
    }

    let addr_clone = addr.clone();

    let mut server_output_rx = {
        let server_running_lock = arc_state.server_running.lock().await;
        let output_tx_lock = arc_state.server_output_tx.lock().await;

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

    let mut retry_interval = tokio::time::interval(tokio::time::Duration::from_secs(2));
    let mut needs_server_status_check = server_output_rx.is_none();
    
    let inner_arc_state = arc_state.clone();
    let inner_out_tx = out_tx.clone();
    tokio::spawn(async move {
        loop {
            check_server(inner_arc_state.clone(), &mut server_output_rx, &mut needs_server_status_check).await;
            let server_msg = async {
                if let Some(rx) = &mut server_output_rx {
                    rx.recv().await
                } else {
                    retry_interval.tick().await;
                    Err(broadcast::error::RecvError::Closed)
                }
            };
            // } => {
            match server_msg.await {
                Ok(msg) => {
                    if let Err(e) = inner_out_tx.send(msg).await {
                        eprintln!("Write error: {}", e);
                    }
                    break;
                    // if let Err(e) = write_half.write_all((msg + "\n").as_bytes()).await {
                    //     eprintln!("[{}] Write error: {}", addr_clone, e);
                    //     break;
                    // }
                }
                Err(broadcast::error::RecvError::Lagged(_)) => {
                    println!("[{}] Lagged behind server output, catching up", addr_clone);
                    continue;
                }
                Err(broadcast::error::RecvError::Closed) => {
                    needs_server_status_check = true;
                } //}
                  //}
            }
        }
    });
    let (mut writer, mut reader) = conn_handler.split().unwrap();

    let mut tick = tokio::time::interval(tokio::time::Duration::from_secs(1));
    tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);

    
    'outer: loop {
        if kill_socket == true {
            println!("Shutting down");
            //write_half.shutdown();
            //kill_socket = false;
            break;
        }
        tokio::select! {
            // Some(msg) = cmd_rx.recv() => {
            //     let payload = serde_json::json!({
            //         "type": "info",
            //         "data": msg,
            //         "authcode": "0"
            //     }).to_string() + "\n";
            //     if let Err(e) = writer.send(payload.as_bytes().to_vec()).await {
            //         eprintln!("[{}] Write error: {}", addr, e);
            //         break;
            //     };
            // }
            Some(out) = out_rx.recv() => {
                if let Err(e) = writer.send((out + "\n").as_bytes().to_vec()).await {
                    eprintln!("[{}] Write error: {}", addr, e);
                    break;
                };
            }

            _ = reader.handle_request(conn_handler) => {
               //println!("got a request");
            },

            _ = tick.tick() => {}

            _ = tokio::time::sleep(tokio::time::Duration::from_secs(120)) => {
                continue;
            }
        }
        // tokio::select! {
        //     Some(out) = out_rx.recv() => {
        //         if let Err(e) = writer.send((out + "\n").as_bytes().to_vec()).await {
        //             eprintln!("[{}] Write error: {}", addr, e);
        //             break;
        //         };
        //     }

        //     _ = reader.handle_request(&mut conn_handler) => {
        //        //println!("got a request");
        //     },
        //     _ = tick.tick() => {}

        //     _ = tokio::time::sleep(tokio::time::Duration::from_secs(120)) => {
        //         continue;
        //     }
        // }
        let inner_arc_state = arc_state.clone();
        loop {
            let mut found_message = false;

            while let Ok(_) = conn_handler.next().await {
                println!("got next");

                let line_str_result = conn_handler.recv_line().await;
                if let Ok(mut line_str) = line_str_result {
                    conn_handler.start_clean_hook().await;
                    line_str = line_str
                        .trim_matches(|c: char| c.is_whitespace() || c == '\0')
                        .to_string();
                    if let Some(stripped) = line_str.strip_prefix(r"\f") {
                        line_str = stripped.to_string();
                        if !line_str.starts_with('{') {
                            line_str = "{".to_owned() + &line_str;
                        }
                    }

                    let router_arc_app_state = inner_arc_state.clone();
                    // if true {
                    if let Ok(json_value) = serde_json::from_str::<Value>(&line_str) {
                        log_requests(json_value.clone(), addr.to_string(), line_str.to_string());

                        let auth_payload_result: Result<AuthTcpMessage, serde_json::Error> =
                            serde_json::from_value(json_value.clone());
                        let authenticated_origins =
                            &mut inner_arc_state.authenticated_origins.lock().await;
                        if let Ok(auth_payload) = auth_payload_result {
                            let node_password: String =
                                get_env_var_or_arg("NODE_PASSWORD", Some(String::default()))
                                    .unwrap();

                            if node_password.clone() == auth_payload.password {
                                authenticated_origins.push(addr.to_string());
                            }
                        }
                        if !authenticated_origins
                            .iter()
                            .any(|origin| *origin == addr.to_string())
                        {
                            let node_password: String =
                                get_env_var_or_arg("NODE_PASSWORD", Some(String::default()))
                                    .unwrap();
                            if !node_password.is_empty() {
                                //kill_socket = true;
                                break 'outer;
                            }
                        }
                        //drop(arc_state_clone);

                        // else if let Ok(request) = FileRequest::from_value(json_value){
                        //     request.execute();
                        // }
                        println!("outside feeding the value");
                        match router.feed_value(json_value).await {
                            Ok(response) => {
                                match response.try_into_response() {
                                    Ok(boxed) => match boxed.downcast::<String>() {
                                        Ok(resp) => println!("got resp: {}", *resp),
                                        Err(boxed) => match boxed.downcast::<Pin<Box<dyn Stream<Item = String> + Send>>>() {
                                            Ok(stream_box) => {
                                                let mut stream = *stream_box;
                                                 let inner_out_tx = out_tx.clone();
                                                tokio::spawn(async move {
                                                    println!("got a stream");
                                                    // let inner_out_tx = out_tx.clone();
                                                    while let Some(item) = stream.next().await {
                                                        println!("stream item: {}", item);
                                                        let _ = inner_out_tx.clone().send(item).await;
                                                        // if let Some(sender) = router_arc_app_state.server_output_tx.lock().await.as_mut() {
                                                        //     println!("sending out");
                                                        //     let _ = sender.send(item);
                                                        // }
                                                    }
                                                });
                                                // while let Some(item) = stream.next().await {
                                                //     println!("stream item: {}", item);
                                                // }
                                            }
                                            Err(_) => println!("resp error: dont know this response type"),
                                        },
                                    },
                                    Err(e) => {
                                        match e {
                                            ExtractorErrors::Err(value) => {
                                                println!("{}", value);
                                            }
                                            _ => {
                                                 println!("resp error: try_into_response failed")
                                            }
                                        }
                                    },
                                }
                            }
                            Err(e) => match e {
                                network_abstraction_lib::RouterErrors::NoHandlerFound => {
                                    println!("no handler found");
                                }
                            },
                        }
                        println!("past feeding the value");
                    } else {
                        println!("got bytes which didnt serialize");
                    }

                    if conn_handler.has_remaining_buffer().await {
                        found_message = true;
                    }
                    conn_handler.end_clean_hook().await;
                } else {
                    println!("getting bytes");
                    let bytes = conn_handler.recv_bytes();
                    if let Ok(request) = FileRequest::from_request(bytes) {
                        println!("got a request");
                        if let Ok(bytes) = request.execute_bytes() {
                            let _ = writer.send(bytes).await;
                        }
                    }
                }
            }
            if !found_message {
                break;
            }
        }
    }
    Ok(())
}
fn spawn_middlewares(router: &mut Router<Arc<AppState>>){
    router.add_middleware(|mapping: String, request: &dyn IntoRequest| {
        if let Some(value_request) = request.as_any().downcast_ref::<ValueRequest>() {
            if let Some(Value::String(message)) = value_request.value.get("message") {
                if *message == mapping {
                    // if *message == "start_server".to_string() {
                    //     MiddlewareAction::SkipPredicate
                    // } else {
                        MiddlewareAction::ReassignValue(request)
                    // }
                } else {
                    MiddlewareAction::Continue
                }
            } else {
                MiddlewareAction::Continue
            }
        } else {
            MiddlewareAction::Continue
        }
    });
} 


async fn ensure_server_directory() {}

// Main function, entrypoint to the program, initalizes the app state, serves a tcp connection
// at the specified and does most of the intial handling of data, including switching between modes (json and file)
// and forwards some messages to other functions to handle command or console data, does health checks and set up the forwarding
// and re-attaching of the server stdin to go back to the main server
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let config_local_url = get_env_var_or_arg("LOCALURL", Some(StaticLocalUrl.to_string()));

    let mut listener = ConnectionManager::serve(config_local_url.clone().unwrap()).await?;
    //TcpListener::bind(config_local_url.clone().unwrap()).await?;
    println!("Listening on {}", config_local_url.unwrap());

    let shared_stdin: Arc<Mutex<Option<ChildStdin>>> = Arc::new(Mutex::new(None));
    // let hostname_ref: Arc<Result<OsString, String>> = Arc::new(match hostname::get() {
    //     Ok(h) => Ok(h),
    //     Err(e) => Err(e.to_string()),
    // });

    let uid = unsafe { libc::getuid() };
    if uid != 0 {
        println!(
            "Not running as root (uid={}). The sandbox WILL NOT work/run",
            uid
        );
    }

    let arc_db = Arc::new(Mutex::new(load_db()));

    // TODO: remove this in favor of it being both written to a persistent db and insertions happening by the server
    let mut db = arc_db.lock().await;
    if !db.server_index.iter().any(|(name, _)| name == "minecraft") {
        db.server_index.insert(
            "minecraft".to_string(),
            ServerIndex::new(
                SERVER_DIR.to_string(),
                "minecraft".to_string(),
                "".to_string(),
                true,
                ServerMetadata::default(),
            ),
        );
        save_db(&db);
    }
    drop(db);

    ensure_server_directory().await;

    // state.cmd_tx = cmd_tx;

    let state = AppState {
        current_server: Arc::new(Mutex::new(None)),
        jailed_user: "server".to_string(),
        authenticated_origins: Arc::new(Mutex::new(Vec::new())),
        server_running: Arc::new(Mutex::new(false)),
        output_tx: Arc::new(Mutex::new(None)),
        cmd_tx: Mutex::new(None),
        cmd_rx: Mutex::new(None),
        stdin_ref: shared_stdin,
        server_output_tx: Arc::new(Mutex::new(None)),
        server_process: Arc::new(Mutex::new(None)),
        last_updated: Arc::new(Mutex::new(None)),
        db_conn: Arc::new(Mutex::new(Some(DbConn::first_connection().await))),
        db: Arc::clone(&arc_db),
    };

    let db_current_server = state.db.lock().await.current_server.clone();
    *state.current_server.lock().await = if !db_current_server.is_empty() {
        Some(db_current_server)
    } else {
        drop(db_current_server);
        None
    };

    let arc_state = Arc::new(state);

    let health_monitor_state = arc_state.clone();

    tokio::spawn(async move {
        let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(5));
        loop {
            interval.tick().await;

            let server_running = health_monitor_state.server_running.lock().await;
            let mut server_process = health_monitor_state.server_process.lock().await;

            if *server_running {
                if let Some(process) = server_process.as_mut() {
                    match process.try_wait() {
                        Ok(Some(_)) => {
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

    let (cmd_tx, mut cmd_rx) = mpsc::channel::<String>(10_000);

    // let conn_state = arc_state;
    *arc_state.cmd_tx.lock().await = Some(Arc::new(cmd_tx.clone()));
    *arc_state.cmd_rx.lock().await = Some(cmd_rx);
    //let conn_state = Mutex::new(arc_state);

    loop {
        //let guard = conn_state.lock().await;
        let mut router = Router::new(Arc::clone(&arc_state));
        //router.register_handler(handler);
        //erase::<_, _, _, _, _>
        router.register_handler(erase_stream_wrapper_result(start_server_handler).mapping("start_server".to_string()));
        router.register_handler(erase_string_wrapper(stop_server_handler).mapping("stop_server".to_string()));
        router.register_handler(erase_string_wrapper(delete_server_handler).mapping("delete_server".to_string()));
        router.register_handler(erase_string_wrapper(set_server_handler).mapping("set_server".to_string()));
        router.register_handler(erase_string_wrapper(set_filter_handler).mapping("set_filter".to_string()));
        router.register_handler(erase_string_wrapper(console_handler).mapping("console".to_string()));
        router.register_handler(erase_string_wrapper(server_data_handler).mapping("server_data".to_string()));
        router.register_handler(erase_string_wrapper(ping_handler).mapping("ping".to_string()));
        router.register_handler(erase_string_wrapper(server_state_handler).mapping("server_state".to_string()));
        router.register_handler(erase_string_wrapper(server_name_handler).mapping("server_name".to_string()));
        router.register_handler(erase_stream_wrapper_result(create_server_handler).mapping("create_server".to_string()));


        let (mut conn_handler, addr_option) = listener.accept_connection().await?;
        let addr = addr_option.unwrap_or("unknown".to_string());
        println!("[Connection] New client from {}", addr);


        //let arc_state_clone = Arc::new(conn_state);

        spawn_middlewares(&mut router);
        tokio::spawn(async move {
            spawn_request_loop(&mut conn_handler, &mut router, addr).await
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

pub fn log_requests(json_value: Value, addr: String, raw_string: String) {
    // This is for logging all json values EXCEPT anything to do with filecontent
    // as if your transfering file content and log that, depending on how big the file it
    // it could crash if that was not filtered
    // it also checks for status messages to filter
    let mut cant_log = false;

    // TODO: serialize these into objects instead of getting from values?
    cant_log = json_value.get("in_response_to").is_some()
        && json_value.get("data").is_some()
        && json_value
            .as_object()
            .map(|o| o.len() == 2)
            .unwrap_or(cant_log);

    if let Ok(payload) = serde_json::from_value::<MessagePayload>(json_value.clone()) {
        if payload.r#type == "server_state" || payload.message == "server_state" {
            cant_log = true;
        }
    }

    if !cant_log {
        println!("[{}] Received JSON here line: {}", addr, raw_string.trim());
    }
}

async fn fix_path(path: String) -> String {
    let server_root = Path::new("server");

    if path.starts_with("server/") || path == "server" {
        let canonical = fs::canonicalize(&path)
            .await
            .unwrap_or_else(|_| server_root.to_path_buf());

        let canonical_server_root = fs::canonicalize(server_root)
            .await
            .unwrap_or_else(|_| server_root.to_path_buf());

        if canonical.starts_with(&canonical_server_root) {
            return canonical.to_string_lossy().into_owned();
        }

        let fixed = server_root.join(path.trim_start_matches("server/"));
        return fixed.to_string_lossy().into_owned();
    }

    let forced = server_root.join(path);

    let canonical_forced = fs::canonicalize(&forced).await.unwrap_or(forced);

    canonical_forced.to_string_lossy().into_owned()
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
                }
                Err(_) => {
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

async fn create_server(
    state: Arc<AppState>,
    cmd_tx: &mpsc::Sender<String>,
    stdin_ref: &Arc<Mutex<Option<ChildStdin>>>,
    payload_raw_value: Value,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    println!("[create_server] called");

    if let Ok(payload) = serde_json::from_value::<IncomingMessageWithMetadata>(payload_raw_value) {
        println!("[create_server] payload deserialized ok");

        if let MetadataTypes::Server {
            servername,
            provider: _,
            location,
            providertype: _,
            sandbox,
            server_metadata: _,
        } = &payload.metadata.clone()
        {
            println!("[create_server] servername={servername:?} location={location:?}");

            let filtered_location = if location.starts_with("server/") {
                location.clone()
            } else {
                format!("server/{}", location)
            };
            println!("[create_server] filtered_location={filtered_location:?}");

            {
                let mut db = state.db.lock().await;
                db.server_index.insert(
                    servername.to_string(),
                    ServerIndex {
                        location: filtered_location.to_string(),
                        provider: {
                            if let MetadataTypes::Server { provider, .. } = &payload.metadata {
                                provider.clone()
                            } else {
                                println!(
                                    "[create_server] provider extraction failed, returning early"
                                );
                                return Ok(());
                            }
                        },
                        providertype: "".to_string(),
                        sandbox: {
                            if let MetadataTypes::Server { sandbox, .. } = &payload.metadata {
                                sandbox.clone()
                            } else {
                                true
                            }
                        },
                        server_metadata: ServerMetadata::default(),
                    },
                );
                save_db(&db);
                println!("[create_server] db updated and saved");
            }

            let provider = {
                if let Some(ProviderTypes::Provider(provider)) = convert_provider(
                    state.clone(),
                    vec![ProviderTypes::Name(servername.clone())],
                    ProviderReturnTypes::Provider,
                )
                .await
                {
                    println!("[create_server] resolved provider={provider:?}");
                    Some(provider)
                } else {
                    println!("[create_server] convert_provider(Provider) returned None");
                    None
                }
            };

            let path = {
                if let Some(ProviderTypes::Path(path)) = convert_provider(
                    state.clone(),
                    vec![ProviderTypes::Name(servername.clone())],
                    ProviderReturnTypes::Path,
                )
                .await
                {
                    println!("[create_server] resolved path={path:?}");
                    Some(path)
                } else {
                    println!("[create_server] convert_provider(Path) returned None");
                    None
                }
            };

            let provider_object = {
                if let Some(ProviderTypes::Object(obj)) = convert_provider(
                    state.clone(),
                    vec![
                        ProviderTypes::Path(path.clone().unwrap_or(String::new())),
                        ProviderTypes::Provider(provider.clone().unwrap_or(String::new())),
                    ],
                    ProviderReturnTypes::Object,
                )
                .await
                {
                    println!("[create_server] resolved provider_object name={:?}", obj.0);
                    Some(obj)
                } else {
                    println!("[create_server] convert_provider(Object) returned None");
                    None
                }
            };

            if let Some((name, provider_platforms)) = provider_object {
                println!("[create_server] provider_object name={name:?}");

                let provider_config = pick_platform(provider_platforms);
                println!(
                    "[create_server] pick_platform result: {:?}",
                    provider_config.is_some()
                );

                let mut prov: ProviderGame = match provider_config.clone() {
                    Some(prov) => prov,
                    None => {
                        println!("[create_server] no platform available, returning error");
                        return Err("No platform".into());
                    }
                }
                .into();

                let set_loc_result = prov.set_location(filtered_location.clone());
                println!(
                    "[create_server] set_location({filtered_location:?}) -> {set_loc_result:?}"
                );

                if let Some(cmd) = prov.pre_hook() {
                    println!("[create_server] running pre_hook: {cmd:?}");
                    let sandbox = resolve_sandbox(&state).await;
                    let path = resolve_path(&state, servername).await;
                    run_command_live_output(
                        &state,
                        cmd,
                        sandbox,
                        path.unwrap_or(String::new()),
                        provider_config.clone().unwrap(),
                        "Pre-hook".into(),
                        Some(cmd_tx.clone()),
                        None,
                        Some(60000),
                    )
                    .await
                    .ok();
                    println!("[create_server] pre_hook done");
                } else {
                    println!("[create_server] no pre_hook");
                }

                if let Some(cmd) = prov.install() {
                    println!("[create_server] running install: {cmd:?}");
                    let sandbox = resolve_sandbox(&state).await;
                    let path = resolve_path(&state, servername).await;
                    run_command_live_output(
                        &state,
                        cmd,
                        sandbox,
                        path.unwrap_or(String::new()),
                        provider_config.clone().unwrap(),
                        "Install".into(),
                        Some(cmd_tx.clone()),
                        None,
                        Some(60000),
                    )
                    .await
                    .ok();
                    println!("[create_server] install done");
                } else {
                    println!("[create_server] no install cmd");
                }

                if let Some(cmd) = prov.post_hook() {
                    println!("[create_server] running post_hook: {cmd:?}");
                    let sandbox = resolve_sandbox(&state).await;
                    let path = resolve_path(&state, servername).await;
                    run_command_live_output(
                        &state,
                        cmd,
                        sandbox,
                        path.unwrap_or(String::new()),
                        provider_config.clone().unwrap(),
                        "Post-hook".into(),
                        Some(cmd_tx.clone()),
                        None,
                        Some(60000),
                    )
                    .await
                    .ok();
                    println!("[create_server] post_hook done");
                } else {
                    println!("[create_server] no post_hook");
                }
            } else {
                println!("[create_server] no provider_object, skipping hooks");
            }

            println!("[create_server] returning Ok");
            Ok(())
        } else {
            println!("[create_server] metadata was not MetadataTypes::Server");
            Ok(())
        }
    } else {
        println!("[create_server] failed to deserialize payload");
        Ok(())
    }
}
async fn resolve_sandbox(state: &Arc<AppState>) -> bool {
    if let Some(ProviderTypes::Sandbox(sandbox)) =
        convert_provider(state.clone(), vec![], ProviderReturnTypes::Sandbox).await
    {
        sandbox
    } else {
        true
    }
}

async fn resolve_path(state: &Arc<AppState>, servername: &str) -> Option<String> {
    if let Some(ProviderTypes::Path(path)) = convert_provider(
        state.clone(),
        vec![ProviderTypes::Name(servername.to_string())],
        ProviderReturnTypes::Path,
    )
    .await
    {
        Some(path)
    } else {
        None
    }
}

fn pick_platform(platform: Platforms) -> Option<ProviderConfig> {
    println!("test");
    if cfg!(target_os = "linux") {
        return platform.linux;
    } else if cfg!(target_os = "windows") {
        return platform.windows;
    } else {
        return None;
    }
}

// use std::process::Command;
const PROVIDER_PATH: &str = "provider-db.json";

// This has been explained in the section for ProviderTypes and ProviderReturnTypes
// but i will recap it here
// an abstraction for now, that manages conversions between
// paths, names, objects, sandbox, etc
// you specify an order of operations, and an expected return type
// if it can find what was asked from some operation (starting from left to right because its a vector/array)
// it will return it, otherwise it will continue onto the next bit of information until it can return the request peice of data
// ProviderTypes is whats inputted and outputted,
// ProviderReturnTypes is what you expect to be returned (does not need an argument)
async fn convert_provider(
    state: Arc<AppState>,
    inputs: Vec<ProviderTypes>,
    expected_output: ProviderReturnTypes,
) -> Option<ProviderTypes> {
    let mut known_name: Option<String> = None;
    let mut known_path: Option<String> = None;
    let mut known_provider: Option<String> = None;
    let mut known_sandbox: Option<bool> = None;
    let mut known_object: Option<(String, Platforms)> = None;

    for input in inputs {
        match input {
            ProviderTypes::Name(name) => {
                known_name.get_or_insert(name);
            }
            ProviderTypes::Path(path) => {
                known_path.get_or_insert(path);
            }
            ProviderTypes::Provider(provider) => {
                known_provider.get_or_insert(provider);
            }
            ProviderTypes::Sandbox(sandbox) => {
                known_sandbox.get_or_insert(sandbox);
            }
            ProviderTypes::Object(object) => {
                known_object.get_or_insert(object);
            }
        }
    }

    if known_path.is_none() {
        if let Some(name) = &known_name {
            if let Some(path) = get_definite_path_from_name(&state, Some(name.clone())).await {
                known_path = Some(path);
            }
        }
    }

    if known_provider.is_none() {
        if let Some(name) = &known_name {
            if let Some(provider) = get_provider_from_servername(&state, Some(name.clone())).await {
                known_provider = Some(provider);
            }
        }
    }

    if known_sandbox.is_none() {
        if let Some(name) = &known_name {
            if let Some(sandbox) =
                get_providers_sandbox(&state, Some(name.clone()), known_path.clone()).await
            {
                known_sandbox = Some(sandbox);
            }
        }
    }

    if known_object.is_none() {
        if let Some(obj) = get_provider_object(
            known_provider.as_deref().or(known_name.as_deref()),
            known_path.as_deref(),
        )
        .await
        {
            known_object = Some(obj);
        }
    }

    let output = match expected_output {
        ProviderReturnTypes::Path => known_path.map(ProviderTypes::Path),
        ProviderReturnTypes::Provider => known_provider.map(ProviderTypes::Provider),
        ProviderReturnTypes::Sandbox => known_sandbox.map(ProviderTypes::Sandbox),
        ProviderReturnTypes::Object => known_object.map(ProviderTypes::Object),
        ProviderReturnTypes::Name => known_name.map(ProviderTypes::Name),
    };

    if output.is_none() {
        println!("Output is None");
    }

    output
}

// Gets a provider out of a handpicked list of gameservers, including custom, at some point needs to be massively re-worked as
// it might be a bit messy having this is my rust code, the majority of the code and types are in provider.rs and it just relies on
// structs I created changing into this provider types, which is one of the few reasons why a better system is needed, it also takes a path to put the files in (not implimented yet)
async fn get_provider_object(
    option_name: Option<&str>,
    option_path: Option<&str>,
) -> Option<(String, Platforms)> {
    if option_name.is_none() || option_path.is_none() {
        println!("returning none");
        return None;
    }
    let (path, name) = (option_path.unwrap(), option_name.unwrap());

    let mut path = path.to_string();
    if !path.starts_with("server/") {
        path = format!("server/{}", path);
        println!("Adjusted path to: {}", path);
    }

    let contents: Result<ProviderDbList, serde_json::Error> =
        serde_json::from_str(&match fs::read_to_string(PROVIDER_PATH).await {
            Ok(prov) => {
                println!("Read provider DB file OK ({} bytes)", prov.len());
                prov
            }
            Err(e) => {
                println!(
                    "ERROR: Failed to read PROVIDER_PATH='{}': {}",
                    PROVIDER_PATH, e
                );
                return None;
            }
        });
    if name == "custom" {
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

                        return Some((name.to_string(), custom.into()));
                    }
                    Err(e) => {
                        println!("ERROR: Failed to parse provider.json: {}", e);
                        return Some((name.to_owned(), Custom::new().into()));
                    }
                }
            }
            Err(e) => {
                println!(
                    "WARN: Could not read provider.json at {}: {}",
                    provider_json_path, e
                );
                return None;
            }
        }
    } else {
        println!("Looking up '{}' in provider DB at: {}", name, PROVIDER_PATH);

        let db = match contents {
            Ok(db) => {
                println!(
                    "Provider DB loaded, entries: {:?}",
                    db.list.keys().collect::<Vec<_>>()
                );
                db
            }
            Err(e) => {
                println!("ERROR: Failed to parse provider DB: {}", e);
                return None;
            }
        };

        if let Some((inner_provider_name, inner_provider)) = db
            .list
            .iter()
            .find(|(provider_name, _)| *provider_name == name)
        {
            println!("found provider: {}", inner_provider_name);
            return Some((inner_provider_name.to_string(), inner_provider.clone()));
        } else {
            println!(
                "'{}' not found in DB. Available: {:?}",
                name,
                db.list.keys().collect::<Vec<_>>()
            );
            return None;
        }
    };
}

// This function soley exists to get whether or not the sandbox is enabled for a specific server
// given either the path or name of the server
async fn get_providers_sandbox(
    state: &AppState,
    option_name: Option<String>,
    option_path: Option<String>,
) -> Option<bool> {
    let db = state.db.lock().await;
    if let Some(name) = option_name {
        if let Some(db_server) = db.server_index.iter().find(|server| *server.0 == name) {
            Some(db_server.1.sandbox)
        } else {
            None
        }
    } else if let Some(path) = option_path {
        if let Some(db_server) = db
            .server_index
            .iter()
            .find(|server| server.1.location == path)
        {
            Some(db_server.1.sandbox)
        } else {
            None
        }
    } else {
        None
    }
}

// Needs to be implimented, provider and servername should not forever remain the same thing and
// a index needs to be kept about what provider matches to which server, the code already store the data
// but at some point i need to decouple the two
async fn get_provider_from_servername(state: &AppState, name: Option<String>) -> Option<String> {
    if name.is_some() {
        let db = state.db.lock().await;
        let server_path = db
            .server_index
            .iter()
            .find(|(server_name, _)| name.clone().unwrap() == **server_name);
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
        let db = state.db.lock().await;
        let server_path = db
            .server_index
            .iter()
            .find(|(server_name, _)| name.clone().unwrap() == **server_name);
        if server_path.is_some() {
            if let Some((_, server_index)) = server_path {
                return Some(server_index.location.clone());
            } else {
                return None;
            }
        } else {
            return None;
        }
    } else {
        return None;
    }
}
