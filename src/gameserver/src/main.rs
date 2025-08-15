use hostname::get;
use serde_json::{json, Value};
use std::convert::TryFrom;
use std::error::Error;
use std::ffi::OsString;
use std::process::{Command, Stdio};
use std::sync::Arc;
use tokio::fs;
use tokio::io;
use tokio::io::{split, AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpListener;
use tokio::process::{ChildStdin, Command as TokioCommand};
use tokio::sync::{mpsc, Mutex};

use tokio::fs::File;
use tokio::io::AsyncRead;
use tokio::net::TcpStream;
use uuid::Uuid;

const SERVER_DIR: &str = "server";

struct Minecraft;

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
enum ApiCalls {
    None,
    Capabilities(Vec<String>),
    NodeList(Vec<String>),
    IncomingMessage(MessagePayload),
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

trait Provider {
    fn pre_hook(&self) -> Option<Command>;
    fn install(&self) -> Option<Command>;
    fn post_hook(&self) -> Option<Command>;
    fn start(&self) -> Option<Command>;
}

impl Provider for Minecraft {
    fn pre_hook(&self) -> Option<Command> {
        if cfg!(target_os = "linux") {
            let mut cmd = Command::new("sh");
            cmd.arg("-c")
                .arg("apt-get update && apt-get install -y libssl-dev pkg-config wget");
            Some(cmd)
        } else if cfg!(target_os = "windows") {
            let mut cmd = Command::new("powershell");
            cmd.arg("-Command").arg("choco install -y wget");
            Some(cmd)
        } else {
            None
        }
    }

    fn install(&self) -> Option<Command> {
        if cfg!(target_os = "linux") {
            let mut cmd = Command::new("sh");
            cmd.arg("-c").arg(
                "apt-get install -y openjdk-17-jre-headless && update-alternatives --set java /usr/lib/jvm/java-17-openjdk-amd64/bin/java"
            );
            Some(cmd)
        } else if cfg!(target_os = "windows") {
            let mut cmd = Command::new("powershell");
            cmd.arg("-Command").arg("choco install -y openjdk");
            Some(cmd)
        } else {
            None
        }
    }

    fn post_hook(&self) -> Option<Command> {
        if cfg!(target_os = "linux") {
            let mut cmd = Command::new("sh");
            cmd.arg("-c").arg(format!(
                "mkdir -p {dir} && cd {dir} && wget -O server.jar https://piston-data.mojang.com/v1/objects/84194a2f286ef7c14ed7ce0090dba59902951553/server.jar && echo 'eula=true' > eula.txt",
                dir = SERVER_DIR
            ));
            Some(cmd)
        } else if cfg!(target_os = "windows") {
            let mut cmd = Command::new("powershell");
            cmd.arg("-Command").arg(format!(
                "New-Item -ItemType Directory -Force -Path {dir}; cd {dir}; Invoke-WebRequest -Uri https://piston-data.mojang.com/v1/objects/84194a2f286ef7c14ed7ce0090dba59902951553/server.jar -OutFile server.jar; 'eula=true' | Out-File -Encoding ASCII eula.txt",
                dir = SERVER_DIR
            ));
            Some(cmd)
        } else {
            None
        }
    }

    fn start(&self) -> Option<Command> {
        if cfg!(target_os = "linux") {
            let mut cmd = Command::new("java");
            cmd.args(&["-Xmx1024M", "-Xms1024M", "-jar", "server.jar", "nogui"])
                .current_dir(SERVER_DIR);
            Some(cmd)
        } else if cfg!(target_os = "windows") {
            let mut cmd = Command::new("java");
            cmd.args(&["-Xmx1024M", "-Xms1024M", "-jar", "server.jar", "nogui"])
                .current_dir(SERVER_DIR);
            Some(cmd)
        } else {
            None
        }
    }
}

#[derive(serde::Serialize)]
struct GetState {
    start_keyword: String,
    stop_keyword: String,
}

fn get_provider(name: &str) -> Option<Minecraft> {
    match name {
        "minecraft" => Some(Minecraft),
        _ => None,
    }
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

#[derive(serde::Serialize, serde::Deserialize)]
struct FileRequestMessage {
    id: u64,
    #[serde(flatten)]
    payload: FileRequestPayload,
}

#[derive(serde::Serialize, serde::Deserialize)]
#[serde(tag = "type", content = "data")]
enum FileRequestPayload {
    Metadata { path: String },
    ListDir { path: String },
}

#[derive(serde::Serialize, serde::Deserialize)]
struct FileResponseMessage {
    in_response_to: u64,
    data: String,
}

#[derive(serde::Serialize, serde::Deserialize)]
struct OneTimeWrapper {
    data: Value,
}

async fn get_metadata(path: &str) -> io::Result<FsMetadata> {
    let metadata = fs::metadata(path).await?;
    let canonical = fs::canonicalize(path).await?;

    let optional_folder_children = if metadata.is_file() {
        None
    } else {
        let mut count = 0;
        let mut dir = fs::read_dir(path).await?;
        while let Some(entry) = dir.next_entry().await? {
            count += 1;
        }
        Some(count)
    };

    Ok(FsMetadata {
        is_file: metadata.is_file(),
        is_dir: metadata.is_dir(),
        optional_folder_children,
        canonical_path: canonical.to_string_lossy().to_string(),
    })
}
async fn list_directory(path: &str) -> std::io::Result<Vec<FsEntry>> {
    let mut entries = Vec::new();
    let mut dir = tokio::fs::read_dir(path).await?;

    while let Some(entry) = dir.next_entry().await? {
        let metadata = entry.metadata().await?;
        entries.push(FsEntry {
            name: entry.file_name().to_string_lossy().to_string(),
            is_file: metadata.is_file(),
            is_dir: metadata.is_dir(),
        });
    }

    Ok(entries)
}


pub async fn handle_multipart_message(
    payload: &MessagePayload,
    current_file: &mut Option<File>,
) -> std::io::Result<()> {
    match payload.r#type.as_str() {
        "start_file" => {
            //Uuid::new_v4()
            let file_name = format!("server/{}", payload.message);

            if let Err(e) = tokio::fs::create_dir_all("server").await {
                eprintln!("Failed to create server directory: {}", e);
            }

            let file = File::create(&file_name).await?;
            *current_file = Some(file);
        }
        "end_file" => {
            if let Some(mut file) = current_file.take() {
                file.flush().await?;
                println!("[handle_multipart_message] Finished writing file");
            } 
        }
        _ => {}
    }
    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let listener = TcpListener::bind(format!("0.0.0.0:{}", PORT)).await?;
    println!("Listening on {}", PORT);

    let verbose = std::env::var("VERBOSE").is_ok();
    let shared_stdin: Arc<Mutex<Option<ChildStdin>>> = Arc::new(Mutex::new(None));
    let hostname: Arc<Result<OsString, String>> = Arc::new(match get() {
        Ok(hostname) => Ok(hostname),
        Err(err) => Err(err.to_string()),
    });

    loop {
        let hostname = Arc::clone(&hostname);
        let (socket, addr) = listener.accept().await?;
        let stdin_ref = shared_stdin.clone();

        println!("New connection from: {}", addr);

        tokio::spawn(async move {
            let (read_half, write_half) = socket.into_split();
            let mut reader = BufReader::new(read_half);
            let mut buf = String::new();
            let mut current_file: Option<File> = None;

            let (out_tx, mut out_rx) = mpsc::channel::<String>(32);
            let (cmd_tx, mut cmd_rx) = mpsc::channel::<String>(32);

            tokio::spawn(async move {
                let mut writer = write_half;
                loop {
                    tokio::select! {
                        Some(msg) = cmd_rx.recv() => {
                            let payload = json!({"type":"info","data":msg,"authcode": "0"}).to_string() + "\n";
                            if let Err(e) = writer.write_all(payload.as_bytes()).await {
                                eprintln!("Write error: {}", e);
                                break;
                            }
                        }
                        Some(out) = out_rx.recv() => {
                            if let Err(e) = writer.write_all((out + "\n").as_bytes()).await {
                                eprintln!("Write error: {}", e);
                                break;
                            }
                        }
                        else => break,
                    }
                }
            });

            loop {
                buf.clear();
                let n = reader.read_line(&mut buf).await;
                if let Ok(0) = n {
                    println!("Connection closed by client: {}", addr);
                    break;
                }
                if let Err(e) = n {
                    eprintln!("Read error from {}: {}", addr, e);
                    break;
                }

                let line = buf.trim_end();

                if line.starts_with('{') {
                    if let Ok(json_line) = serde_json::from_str::<Value>(&line) {
                        if let Ok(payload) =
                            serde_json::from_value::<MessagePayload>(json_line.clone())
                        {
                            if payload.r#type == "start_file" {
                                println!("Processing file message: {:?}", payload);
                                if let Err(e) =
                                    handle_multipart_message(&payload, &mut current_file).await
                                {
                                    eprintln!("Error handling multipart message: {}", e);
                                }
                                continue;
                            }
                        }

                        if let Ok(request) =
                            serde_json::from_value::<FileRequestMessage>(json_line.clone())
                        {
                            let out_tx_clone = out_tx.clone();
                            tokio::spawn(async move {
                                let response_json = match request.payload {
                                    FileRequestPayload::Metadata { path } => {
                                        match get_metadata(&path).await {
                                            Ok(metadata) => {
                                                let metadata_json =
                                                    serde_json::to_string(&metadata)
                                                        .unwrap_or_else(|_| "{}".to_string());

                                                let response_msg = FileResponseMessage {
                                                    in_response_to: request.id,
                                                    data: metadata_json,
                                                };

                                                serde_json::to_string(&response_msg).unwrap_or_else(
                                                    |_| {
                                                        r#"{"in_response_to":0,"data":""}"#
                                                            .to_string()
                                                    },
                                                )
                                            }
                                            Err(e) => {
                                                eprintln!("TcpFs get_metadata error: {}", e);
                                                let error_response = FileResponseMessage {
                                                    in_response_to: request.id,
                                                    data: format!(r#"{{"error":"{}"}}"#, e),
                                                };
                                                serde_json::to_string(&error_response)
                                                    .unwrap_or_else(|_| {
                                                        r#"{"in_response_to":0,"data":""}"#
                                                            .to_string()
                                                    })
                                            }
                                        }
                                    }
                                    FileRequestPayload::ListDir { path } => {
                                        match list_directory(&path).await {
                                            Ok(entries) => {
                                                let entries_json = serde_json::to_string(&entries)
                                                    .unwrap_or_else(|_| "[]".to_string());

                                                let response_msg = FileResponseMessage {
                                                    in_response_to: request.id,
                                                    data: entries_json,
                                                };

                                                serde_json::to_string(&response_msg).unwrap_or_else(
                                                    |_| {
                                                        r#"{"in_response_to":0,"data":""}"#
                                                            .to_string()
                                                    },
                                                )
                                            }
                                            Err(e) => {
                                                eprintln!("TcpFs list_directory error: {}", e);
                                                let error_response = FileResponseMessage {
                                                    in_response_to: request.id,
                                                    data: format!(r#"{{"error":"{}"}}"#, e),
                                                };
                                                serde_json::to_string(&error_response)
                                                    .unwrap_or_else(|_| {
                                                        r#"{"in_response_to":0,"data":""}"#
                                                            .to_string()
                                                    })
                                            }
                                        }
                                    }
                                };

                                if let Err(e) = out_tx_clone.send(response_json).await {
                                    eprintln!("Failed to send TcpFs response: {}", e);
                                }
                            });
                            continue;
                        }

                        if let Ok(val) = serde_json::from_value::<MessagePayload>(json_line.clone())
                        {
                            let typ = val.r#type.clone();
                            if typ == "command" {
                                let cmd_str = val.message.clone();
                                if cmd_str == "create_server" {
                                    if let Some(prov) = get_provider("minecraft") {
                                        if let Some(cmd) = prov.pre_hook() {
                                            let _ = run_command_live_output(
                                                cmd,
                                                "Pre-hook".into(),
                                                Some(cmd_tx.clone()),
                                                None,
                                            )
                                            .await;
                                        }
                                        if let Some(cmd) = prov.install() {
                                            let _ = run_command_live_output(
                                                cmd,
                                                "Install".into(),
                                                Some(cmd_tx.clone()),
                                                None,
                                            )
                                            .await;
                                        }
                                        if let Some(cmd) = prov.post_hook() {
                                            let _ = run_command_live_output(
                                                cmd,
                                                "Post-hook".into(),
                                                Some(cmd_tx.clone()),
                                                None,
                                            )
                                            .await;
                                        }
                                        if let Some(cmd) = prov.start() {
                                            println!("starting");
                                            let tx = cmd_tx.clone();
                                            let stdin_clone = stdin_ref.clone();
                                            tokio::spawn(async move {
                                                let _ = run_command_live_output(
                                                    cmd,
                                                    "Server".into(),
                                                    Some(tx),
                                                    Some(stdin_clone),
                                                )
                                                .await;
                                            });
                                            let _ = cmd_tx.send("Server started".into()).await;
                                        }
                                    }
                                } else if cmd_str == "stop_server" {
                                    let input = "stop";
                                    let mut guard = stdin_ref.lock().await;
                                    if let Some(stdin) = guard.as_mut() {
                                        let _ = stdin
                                            .write_all(format!("{}\n", input).as_bytes())
                                            .await;
                                        let _ = stdin.flush().await;
                                        let _ =
                                            cmd_tx.send(format!("Sent to server: {}", input)).await;
                                    }
                                } else if cmd_str == "start_server" {
                                    if let Some(prov) = get_provider("minecraft") {
                                        if let Some(cmd) = prov.start() {
                                            println!("starting");
                                            let tx = cmd_tx.clone();
                                            let stdin_clone = stdin_ref.clone();
                                            tokio::spawn(async move {
                                                let _ = run_command_live_output(
                                                    cmd,
                                                    "Server".into(),
                                                    Some(tx),
                                                    Some(stdin_clone),
                                                )
                                                .await;
                                            });
                                            let _ = cmd_tx.send("Server started".into()).await;
                                        }
                                    }
                                } else if cmd_str == "server_data" {
                                    let _ = cmd_tx
                                        .send(
                                            serde_json::to_string(&GetState {
                                                start_keyword: "help".to_string(),
                                                stop_keyword: "All dimensions are saved"
                                                    .to_string(),
                                            })
                                            .expect("Failed, not json"),
                                        )
                                        .await;
                                } else if cmd_str == "server_name" {
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
                                            .expect("Failed, not json"),
                                        )
                                        .await;
                                } else {
                                    let _ =
                                        cmd_tx.send(format!("Unknown command: {}", cmd_str)).await;
                                }
                            } else if typ == "console" {
                                let input = val.message;
                                let mut guard = stdin_ref.lock().await;
                                if let Some(stdin) = guard.as_mut() {
                                    let _ =
                                        stdin.write_all(format!("{}\n", input).as_bytes()).await;
                                    let _ = stdin.flush().await;
                                    let _ = cmd_tx.send(format!("Sent to server: {}", input)).await;
                                }
                            }
                        } else if let Ok(_val) = json_line.clone().try_into() as Result<List, _> {
                            println!("Received capabilities");
                            let _ = out_tx
                                .send(
                                    serde_json::to_string(&List {
                                        list: vec!["all".to_string()],
                                    })
                                    .unwrap(),
                                )
                                .await;
                        }
                    }
                } else if !line.is_empty() {
                    if let Some(file) = &mut current_file {
                        if let Err(e) = file.write_all(line.as_bytes()).await {
                            eprintln!("Error writing to file: {}", e);
                        } else {
                            println!("[file_upload] Writing {} bytes to current file", line.len());
                        }
                        if let Err(e) = file.write_all(b"\n").await {
                            eprintln!("Error writing newline to file: {}", e);
                        }
                    } else {
                        let mut guard = stdin_ref.lock().await;
                        if let Some(stdin) = guard.as_mut() {
                            let _ = stdin.write_all(format!("{}\n", line).as_bytes()).await;
                            let _ = stdin.flush().await;
                            let _ = cmd_tx.send(format!("Sent to server: {}", line)).await;
                        } else {
                            let _ = cmd_tx.send("Server not running".into()).await;
                        }
                    }
                }
            }

            if let Some(mut file) = current_file.take() {
                let _ = file.flush().await;
                println!("Connection closed, flushed any open file");
            }
        });
    }
}
