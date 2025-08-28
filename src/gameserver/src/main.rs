use hostname::get;
use serde_json::{json, Value};
use std::convert::TryFrom;
use std::error::Error;
use std::ffi::OsString;
use std::io::SeekFrom;
use std::process::{Command, Stdio};
use std::sync::Arc;
use tokio::fs;
use tokio::fs::OpenOptions;
use tokio::io;
use tokio::io::{split, AsyncBufReadExt, AsyncReadExt, AsyncSeekExt, AsyncWriteExt, BufReader};
use tokio::net::TcpListener;
use tokio::process::{ChildStdin, Command as TokioCommand};
use tokio::sync::{mpsc, Mutex};

use crate::broadcast::Sender;
use crate::filesystem::cleanup_end_file_markers;
use crate::filesystem::send_folder_over_broadcast;
use crate::filesystem::BasicPath;

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

mod extra;
mod filesystem;
mod providers;

const ENABLE_BROADCAST_LOGS: bool = true;
const SERVER_DIR: &str = "server";

#[derive(Debug, serde::Serialize, serde::Deserialize, Clone)]
struct IncomingMessage {
    message: String,
    #[serde(rename = "type")]
    message_type: String,
    authcode: String,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
struct IncomingMessageWithMetadata {
    message: String,
    #[serde(rename = "type")]
    message_type: String,
    metadata: MetadataTypes,
    authcode: String,
}

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

#[derive(Debug, serde::Serialize, serde::Deserialize, Clone)]
struct SrcAndDest {
    src: ApiCalls,
    dest: ApiCalls,
    metadata: String,
}

#[derive(Debug, serde::Serialize, serde::Deserialize, Clone, PartialEq)]
#[serde(tag = "kind", content = "data")]
pub enum NodeStatus {
    Enabled,
    Disabled,
    ImmutablyEnabled,
    ImmutablyDisabled,
}

#[derive(Debug, serde::Serialize, serde::Deserialize, Clone, PartialEq)]
#[serde(tag = "kind", content = "data")]
pub enum NodeType {
    Custom,
    Main,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct Node {
    pub nodename: String,
    pub ip: String,
    pub nodetype: NodeType,
    pub nodestatus: NodeStatus,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
struct List {
    list: Vec<String>,
}

#[derive(Debug, serde::Serialize, serde::Deserialize, Clone)]
struct MessagePayload {
    r#type: String,
    message: String,
    authcode: String,
}

#[derive(Debug, serde::Deserialize, serde::Serialize, Clone)]
#[serde(tag = "kind", content = "data")]
enum ApiCalls {
    None,
    Capabilities(Vec<String>),
    NodeList(Vec<String>),
    IncomingMessage(MessagePayload),
    Node(Node),
}

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

#[cfg(feature = "full-stack")]
static PORT: &str = "8080";

#[cfg(not(feature = "full-stack"))]
static PORT: &str = "8082";

#[derive(serde::Serialize)]
struct GetState {
    start_keyword: String,
    stop_keyword: String,
}

async fn run_command_live_output(
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

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct FsMetadata {
    pub is_file: bool,
    pub is_dir: bool,
    pub optional_folder_children: Option<u64>,
    pub canonical_path: String,
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct FsEntry {
    pub name: String,
    pub is_file: bool,
    pub is_dir: bool,
}

#[derive(Debug, serde::Serialize, serde::Deserialize, Clone)]
pub struct FileChunk {
    file_name: String,
    file_chunk_offet: String,
    file_chunk_size: String,
}

#[derive(serde::Serialize, serde::Deserialize, Debug)]
struct FileRequestMessage {
    id: u64,
    #[serde(flatten)]
    payload: FileRequestPayload,
}

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

#[derive(serde::Serialize, serde::Deserialize)]
struct FileResponseMessage {
    in_response_to: u64,
    data: serde_json::Value,
}

enum ReadMode {
    Json,
    MigrationFile {
        current_file: tokio::fs::File,
        file_name: String,
        bytes_written: u64,
    },
    NormalFile {
        current_file: tokio::fs::File,
    },
}

#[derive(Clone)]
struct AppState {
    current_server: String,
    server_running: Arc<Mutex<bool>>,
}

#[derive(serde::Serialize, serde::Deserialize)]
struct OneTimeWrapper {
    data: Value,
}

async fn get_metadata(path: &str) -> io::Result<FsMetadata> {
    let metadata = fs::metadata(path).await?;
    let canonical = fs::canonicalize(path).await?;

    let optional_folder_children = if metadata.is_dir() {
        let mut count = 0;
        let mut dir = fs::read_dir(path).await?;
        while let Some(entry) = dir.next_entry().await? {
            count += 1;
        }
        Some(count)
    } else {
        None
    };

    Ok(FsMetadata {
        is_file: metadata.is_file(),
        is_dir: metadata.is_dir(),
        optional_folder_children,
        canonical_path: canonical.to_string_lossy().to_string(),
    })
}

async fn list_directory_with_range(
    path: &str,
    start: Option<u64>,
    end: Option<u64>,
) -> io::Result<Vec<FsEntry>> {
    let mut entries = Vec::new();
    let mut dir = fs::read_dir(path).await?;

    let start_idx = start.unwrap_or(0);
    let mut current_idx = 0u64;

    while let Some(entry) = dir.next_entry().await? {
        if current_idx < start_idx {
            current_idx += 1;
            continue;
        }

        if let Some(end_idx) = end {
            if current_idx >= end_idx {
                break;
            }
        }

        let metadata = entry.metadata().await?;
        entries.push(FsEntry {
            name: entry.file_name().to_string_lossy().to_string(),
            is_file: metadata.is_file(),
            is_dir: metadata.is_dir(),
        });

        current_idx += 1;
    }

    Ok(entries)
}

async fn list_directory(path: &str) -> io::Result<Vec<FsEntry>> {
    list_directory_with_range(path, None, None).await
}
pub async fn get_files_content(file_chunk: FileChunk) -> io::Result<MessagePayload> {
    let metadata = fs::metadata(&file_chunk.file_name).await?;

    if metadata.is_dir() {
        return Err(io::Error::new(
            io::ErrorKind::Other,
            format!("Path is a directory: {}", file_chunk.file_name),
        ));
    }

    let mut file = File::open(&file_chunk.file_name).await?;
    let offset: u64 = file_chunk
        .file_chunk_offet
        .parse()
        .expect("Invalid file offset");

    file.seek(SeekFrom::Start(offset.try_into().unwrap()))
        .await?;
    let chunk_size: usize = file_chunk
        .file_chunk_size
        .parse()
        .expect("Invalid chunk size");

    let mut buffer = vec![0; chunk_size];
    let bytes_read = file.read(&mut buffer).await?;
    let content = String::from_utf8_lossy(&buffer[..bytes_read]).to_string();

    Ok(MessagePayload {
        r#type: "file_content".to_string(),
        message: content,
        authcode: "0".to_string(),
    })
}

pub async fn handle_multipart_message(
    payload: &MessagePayload,
    current_file: &mut Option<File>,
) -> std::io::Result<()> {
    match payload.r#type.as_str() {
        "start_file" => {
            let file_name = format!("server/{}", payload.message);
            tokio::fs::create_dir_all("server").await?;
            let file = OpenOptions::new()
                .create(true)
                .write(true)
                .open(&file_name)
                .await?;
            *current_file = Some(file);
            println!(
                "[handle_multipart_message] Started writing file: {}",
                file_name
            );
        }
        "end_file" => {
            if let Some(mut file) = current_file.take() {
                file.flush().await?;
                println!(
                    "[handle_multipart_message] Finished writing file: {}",
                    payload.message
                );
            } else {
                eprintln!(
                    "[handle_multipart_message] end_file received but no file is currently open"
                );
            }
        }
        _ => {}
    }
    Ok(())
}

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

pub async fn handle_incoming(mut conn: tokio::net::TcpStream) -> anyhow::Result<()> {
    let mut reader = BufReader::new(&mut conn);
    let mut line = String::new();

    let mut current_file: Option<File> = None;

    loop {
        line.clear();
        let n = reader.read_line(&mut line).await?;
        if n == 0 {
            break;
        }

        if let Ok(json) = serde_json::from_str::<serde_json::Value>(&line) {
            match json.get("type").and_then(|t| t.as_str()) {
                Some("start_file") => {
                    let path = json.get("message").and_then(|m| m.as_str()).unwrap();
                    current_file = Some(File::create(path).await?);
                    eprintln!("[handle_file] Started writing file: {}", path);
                }
                Some("end_file") => {
                    eprintln!("[handle_file] Finished file.");
                    current_file = None;
                }
                _ => {
                    eprintln!("JSON received but no handler matched: {:?}", json);
                }
            }
        } else if let Some(file) = &mut current_file {
            file.write_all(line.as_bytes()).await?;
        } else {
            eprintln!("Invalid JSON received: {:?}", line);
        }
    }

    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    const FILE_DELIMITER: &[u8] = b"<|END_OF_FILE|>";
    const PORT: u16 = 8082;

    let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{}", PORT)).await?;
    println!("Listening on {}", PORT);

    let shared_stdin: Arc<Mutex<Option<ChildStdin>>> = Arc::new(Mutex::new(None));
    let hostname_ref: Arc<Result<OsString, String>> = Arc::new(match hostname::get() {
        Ok(h) => Ok(h),
        Err(e) => Err(e.to_string()),
    });

    let state = AppState {
        current_server: "minecraft".to_string(),
        server_running: Arc::new(Mutex::new(false)),
    };
    let arc_state = Arc::new(state);

    loop {
        let (socket, addr) = listener.accept().await?;
        println!("[Connection] New client from {}", addr);

        let stdin_ref = shared_stdin.clone();
        let hostname_ref = hostname_ref.clone();
        let arc_state_clone = arc_state.clone();

        tokio::spawn(async move {
            let (mut read_half, mut write_half) = socket.into_split();
            let (out_tx, mut out_rx) = mpsc::channel::<String>(128);
            let (cmd_tx, mut cmd_rx) = mpsc::channel::<String>(128);

            tokio::spawn(async move {
                loop {
                    tokio::select! {
                        Some(msg) = cmd_rx.recv() => {
                            let payload = serde_json::json!({
                                "type": "info",
                                "data": msg,
                                "authcode": "0"
                            }).to_string() + "\n";
                            if let Err(e) = write_half.write_all(payload.as_bytes()).await {
                                eprintln!("[{}] Write error: {}", addr, e);
                                break;
                            }
                        }
                        Some(out) = out_rx.recv() => {
                            if let Err(e) = write_half.write_all((out + "\n").as_bytes()).await {
                                eprintln!("[{}] Write error: {}", addr, e);
                                break;
                            }
                        }
                        else => break,
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

                                if line_str.trim().starts_with('{')
                                    && line_str.trim().ends_with('}')
                                {
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
                                            }
                                        } else if let Ok(msg_payload) =
                                            serde_json::from_value::<IncomingMessageWithMetadata>(
                                                json_value.clone(),
                                            )
                                        {
                                            handle_commands_with_metadata(
                                                &Arc::clone(&arc_state_clone),
                                                &msg_payload,
                                                &cmd_tx,
                                                &stdin_ref,
                                                &hostname_ref,
                                            )
                                            .await;
                                        } else if let Ok(msg_payload) =
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
                                                _ => {
                                                    handle_command_or_console(
                                                        &Arc::clone(&arc_state_clone),
                                                        &msg_payload,
                                                        &cmd_tx,
                                                        &stdin_ref,
                                                        &hostname_ref,
                                                    )
                                                    .await;
                                                }
                                            }
                                        }
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

async fn handle_file_request(state: &Arc<AppState>, request: FileRequestMessage) -> String {
    match request.payload {
        FileRequestPayload::Metadata { path } => match get_metadata(&path).await {
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
            let basic_path_response = BasicPath { paths: vec![] };
            serde_json::to_string(&FileResponseMessage {
                in_response_to: request.id,
                data: serde_json::to_value(basic_path_response).unwrap(),
            })
            .unwrap()
        }
        FileRequestPayload::ListDir { path } => match list_directory(&path).await {
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
            match list_directory_with_range(&path, start, end).await {
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

fn drain_line(read_buf: &mut Vec<u8>, newline_pos: usize) {
    if newline_pos + 1 <= read_buf.len() {
        read_buf.drain(..newline_pos + 1);
    } else {
        read_buf.clear();
    }
}

async fn finish_file(
    current_file: &mut tokio::fs::File,
    file_name: &str,
    bytes_written: u64,
    addr: std::net::SocketAddr,
) {
    if let Err(e) = current_file.flush().await {
        eprintln!("[{}] Error flushing {}: {}", addr, file_name, e);
    } else {
        println!(
            "[{}] Received file: {} ({} bytes)",
            addr, file_name, bytes_written
        );
    }
}

async fn write_file_chunk(
    read_buf: &mut Vec<u8>,
    current_file: &mut tokio::fs::File,
    bytes_written: &mut u64,
    chunk_size: usize,
    addr: std::net::SocketAddr,
    file_name: &str,
) {
    let write_size = read_buf.len().min(chunk_size);

    if write_size > 0 {
        if let Err(e) = current_file.write_all(&read_buf[..write_size]).await {
            eprintln!("[{}] Error writing to {}: {}", addr, file_name, e);
            return;
        }

        *bytes_written += write_size as u64;

        if write_size <= read_buf.len() {
            read_buf.drain(..write_size);
        } else {
            read_buf.clear();
        }
    }
}

fn find_subsequence(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.is_empty() || haystack.len() < needle.len() {
        return None;
    }

    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}
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

                    if message_count % 1000 == 0 || total_bytes_written % 10_000_000 == 0 {
                        println!(
                            "[tcp_to_writer] Wrote {} messages, {} total bytes",
                            message_count, total_bytes_written
                        );
                    }
                }
                Err(e) => {
                    eprintln!("[tcp_to_writer] Failed to write message {} ({} bytes) to socket after {} total bytes: {}", 
                            message_count, msg_len, total_bytes_written, e);
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

async fn handle_commands_with_metadata(
    state: &AppState,
    payload: &IncomingMessageWithMetadata,
    cmd_tx: &mpsc::Sender<String>,
    stdin_ref: &Arc<Mutex<Option<ChildStdin>>>,
    hostname: &Arc<Result<OsString, String>>,
) {
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
                    if let Some(prov) = get_provider(
                        providertype,
                        &get_definite_path_from_tag(
                            state,
                            get_provider_from_servername(&state, state.current_server.clone())
                                .await,
                        )
                        .await,
                    ) {
                        if let Some(cmd) = prov.pre_hook() {
                            run_command_live_output(
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
                    }
                }
            }
            _ => {}
        }
    }
}
fn get_provider(name: &str, pre_path: &str) -> Option<ProviderType> {
    let mut path = pre_path.to_string();
    if !path.starts_with("server/") {
        path = format!("server/{}", path);
        println!("[DEBUG] Adjusted path to: {}", path);
    }

    match name {
        "minecraft" => {
            println!("[INFO] Selected provider: Minecraft");
            Some(Minecraft.into())
        }
        "custom" => {
            let provider_json_path = format!("{}/provider.json", path);
            println!(
                "[INFO] Looking for custom provider config at: {}",
                provider_json_path
            );

            match std::fs::read_to_string(&provider_json_path) {
                Ok(json_content) => {
                    println!("[DEBUG] Successfully read provider.json");
                    match serde_json::from_str::<ProviderConfig>(&json_content) {
                        Ok(config) => {
                            println!("[INFO] Loaded custom provider config successfully");
                            let mut custom = Custom::new();

                            if let Some(cmd) = config.pre_hook {
                                println!("[INFO] Adding pre_hook: {}", cmd);
                                custom = custom.with_pre_hook(cmd);
                            }
                            if let Some(cmd) = config.install {
                                println!("[INFO] Adding install: {}", cmd);
                                custom = custom.with_install(cmd);
                            }
                            if let Some(cmd) = config.post_hook {
                                println!("[INFO] Adding post_hook: {}", cmd);
                                custom = custom.with_post_hook(cmd);
                            }
                            if let Some(cmd) = config.start {
                                println!("[INFO] Adding start: {}", cmd);
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

async fn get_provider_from_servername(state: &AppState, servername: String) -> String {
    servername
}
async fn get_definite_path_from_tag(state: &AppState, tag: String) -> String {
    String::new()
}
async fn handle_command_or_console(
    state: &AppState,
    payload: &MessagePayload,
    cmd_tx: &mpsc::Sender<String>,
    stdin_ref: &Arc<Mutex<Option<ChildStdin>>>,
    hostname: &Arc<Result<OsString, String>>,
) {
    let typ = payload.r#type.clone();
    if typ == "command" {
        let cmd_str = payload.message.clone();
        match cmd_str.as_str() {
            "delete_server" => {
                let mut path = get_definite_path_from_tag(
                    state,
                    get_provider_from_servername(&state, state.current_server.clone()).await,
                )
                .await;

                if !path.starts_with("server/") {
                    path = format!("server/{}", path);
                }
                if let Err(errro) = fs::remove_dir_all(&path).await {
                    eprintln!("Failed to delete directory {}: {}", path, errro);
                } else {
                    if let Err(errro) = fs::create_dir(&path).await {
                        eprintln!("Failed to recreate directory {}: {}", path, errro);
                    } else {
                        println!("Successfully cleared directory: {}", path);
                    }
                }
            }
            "stop_server" => {
                if let Some(prov) = get_provider(
                    &get_provider_from_servername(&state, state.current_server.clone()).await,
                    &get_definite_path_from_tag(
                        state,
                        get_provider_from_servername(&state, state.current_server.clone()).await,
                    )
                    .await,
                ) {
                    let input = "stop";
                    let mut guard = stdin_ref.lock().await;
                    if let Some(stdin) = guard.as_mut() {
                        let _ = stdin.write_all(format!("{}\n", input).as_bytes()).await;
                        let _ = stdin.flush().await;
                        let _ = cmd_tx.send(format!("Sent to server: {}", input)).await;
                    }
                }
            }
            "start_server" => {
                // Check if server is already running
                {
                    let stdin_guard = stdin_ref.lock().await;
                    if stdin_guard.is_some() {
                        let _ = cmd_tx
                            .send("Server is already running. Use 'stop_server' first.".into())
                            .await;
                        return;
                    }
                }

                if let Some(prov) = get_provider(
                    &get_provider_from_servername(&state, state.current_server.clone()).await,
                    &get_definite_path_from_tag(
                        state,
                        get_provider_from_servername(&state, state.current_server.clone()).await,
                    )
                    .await,
                ) {
                    if let Some(cmd) = prov.start() {
                        let tx = cmd_tx.clone();
                        let stdin_clone = stdin_ref.clone();

                        tokio::spawn(async move {
                            let result = run_command_live_output(
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
                    } else {
                        let _ = cmd_tx
                            .send("No start command available for this provider".into())
                            .await;
                    }
                } else {
                    let _ = cmd_tx
                        .send("Failed to get provider for server".into())
                        .await;
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
            }
            other => {
                let _ = cmd_tx.send(format!("Unknown command: {}", other)).await;
            }
        }
    } else if typ == "console" {
        let input = payload.message.clone();
        let mut guard = stdin_ref.lock().await;
        if let Some(stdin) = guard.as_mut() {
            let _ = stdin.write_all(format!("{}\n", input).as_bytes()).await;
            let _ = stdin.flush().await;
            let _ = cmd_tx.send(format!("Sent to server: {}", input)).await;
        }
    }
}
