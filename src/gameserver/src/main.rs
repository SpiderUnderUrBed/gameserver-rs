use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader, split};
use serde_json::{Value, json};
use tokio::sync::{mpsc, Mutex};
use tokio::net::TcpListener;
use tokio::process::{Command as TokioCommand, ChildStdin};
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::convert::TryFrom;
use std::error::Error;
use std::ffi::OsString;
use hostname::get;

const SERVER_DIR: &str = "server";

struct Minecraft;

#[derive(Serialize, Deserialize)]
struct FileRequestMessage {
    id: u64,
    #[serde(flatten)]
    payload: FileRequestPayload,
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
enum FileRequestPayload {
    Metadata { path: String },
    ListDir { path: String },
}

#[derive(Serialize, Deserialize)]
struct FileResponseMessage {
    in_response_to: u64,
    data: Vec<u8>,
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
// #[serde(tag = "kind", content = "data")]
enum ApiCalls {
    None,
    Capabilities(Vec<String>),
    NodeList(Vec<String>),
    //ButtonList(Vec<Button>),
    IncomingMessage(MessagePayload),
}


impl TryFrom<Value> for List {
    type Error = &'static str;

    fn try_from(value: Value) -> Result<Self, Self::Error> {
        if let Some(full_struct) = value.get("list") {
            if let Some(Value::Array(list)) = full_struct.get("data") {
                    return Ok(List { list: list.iter()
                            .filter_map(|v| v.as_str().map(|s| s.to_string()))
                            .collect()
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
            cmd.arg("-c").arg("apt-get update && apt-get install -y libssl-dev pkg-config wget");
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
    stop_keyword: String
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
    tokio_cmd.stdout(Stdio::piped()).stderr(Stdio::piped()).stdin(Stdio::piped());
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
                    let msg = json!({"type":"stdout","data":format!("[{}] {}", lbl, line)}).to_string();
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
                    let msg = json!({"type":"stderr","data":format!("[{}] {}", lbl, line)}).to_string();
                    let _ = tx.send(msg).await;
                }
            }
        });
    }

    let status = child.wait().await?;
    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let listener = TcpListener::bind(format!("0.0.0.0:{}", PORT)).await?;
    println!("Listening on {}", PORT);

    let verbose = std::env::var("VERBOSE").is_ok();
    let shared_stdin: Arc<Mutex<Option<ChildStdin>>> = Arc::new(Mutex::new(None));
    let hostname: Arc<Result<OsString, String>> = Arc::new(match hostname::get() {
        Ok(hostname) => Ok(hostname),
        Err(err) => Err(err.to_string()),
    });

    loop {
        let hostname = Arc::clone(&hostname);
        let (socket, addr) = listener.accept().await?;
        let stdin_ref = shared_stdin.clone();

        tokio::spawn(async move {
            let (read_half, write_half) = socket.into_split();
            let mut reader = BufReader::new(read_half);
            let mut writer = write_half;
            let mut buf = String::new();

            let (cmd_tx, mut cmd_rx) = mpsc::channel::<FileResponseMessage>(32);

            // Response handler task
            tokio::spawn(async move {
                while let Some(response) = cmd_rx.recv().await {
                    if let Ok(json) = serde_json::to_string(&response) {
                        let _ = writer.write_all((json + "\n").as_bytes()).await;
                    }
                }
            });

            loop {
                buf.clear();
                let n = reader.read_line(&mut buf).await;
                if let Ok(0) = n {
                    break;
                }
                if let Err(e) = n {
                    eprintln!("Read error: {}", e);
                    break;
                }

                let line = buf.trim_end();
                if line.starts_with('{') {
                    match serde_json::from_str::<FileRequestMessage>(line) {
                        Ok(request) => {
                            let response = match request.payload {
                                FileRequestPayload::Metadata { path } => {
                                    // Handle metadata request
                                    FileResponseMessage {
                                        in_response_to: request.id,
                                        data: serde_json::to_vec(&FsMetadata {
                                            is_file: false,
                                            is_dir: true,
                                            canonical_path: path,
                                        })?,
                                    }
                                }
                                FileRequestPayload::ListDir { path } => {
                                    // Handle directory listing
                                    FileResponseMessage {
                                        in_response_to: request.id,
                                        data: serde_json::to_vec(&vec![
                                            FsEntry {
                                                name: "file1.txt".to_string(),
                                                is_file: true,
                                                is_dir: false,
                                            },
                                            FsEntry {
                                                name: "folder1".to_string(),
                                                is_file: false,
                                                is_dir: true,
                                            },
                                        ])?,
                                    }
                                }
                            };

                            if let Err(e) = cmd_tx.send(response).await {
                                eprintln!("Failed to send response: {}", e);
                            }
                        }
                        Err(e) => {
                            eprintln!("Failed to parse request: {}", e);
                        }
                    }
                } else if !line.is_empty() {
                    // Handle legacy commands
                    let response = if line == "create_server" {
                        // Server creation logic...
                        FileResponseMessage {
                            in_response_to: 0, // No ID for legacy commands
                            data: serde_json::to_vec("Server created")?,
                        }
                    } else {
                        FileResponseMessage {
                            in_response_to: 0,
                            data: serde_json::to_vec("Unknown command")?,
                        }
                    };

                    if let Err(e) = cmd_tx.send(response).await {
                        eprintln!("Failed to send response: {}", e);
                    }
                }
            }
        });
    }
}