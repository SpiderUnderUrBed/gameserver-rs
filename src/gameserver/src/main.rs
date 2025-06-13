use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader, split};
use serde_json::{Value, json};
use tokio::sync::{mpsc, Mutex};
use tokio::net::TcpListener;
use tokio::process::{Command as TokioCommand, ChildStdin};
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::convert::TryFrom;

// use serde::Deserializer;
// use serde::Deserialize;
//
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
// #[serde(tag = "kind", content = "data")]
enum ApiCalls {
    None,
    Capabilities(Vec<String>),
    NodeList(Vec<String>),
    IncomingMessage(MessagePayload),
}
// Value::deserialize(
// impl<'de> Deserialize<'de> for ApiCalls {
//     fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
//     where
//         D: Deserializer<'de>,
//     {
//         let value = Value::deserialize(deserializer)?;

//         if let Ok(direct) = serde_json::from_value::<ApiCalls>(value.clone()) {
//             return Ok(direct);
//         }

//         if let Some(inner) = value.get("list") {
//             if let Ok(wrapped) = serde_json::from_value::<ApiCalls>(inner.clone()) {
//                 return Ok(wrapped);
//             }
//         }

//         Err(serde::de::Error::custom("Invalid ApiCalls format"))
//     }
// }


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

    loop {
        let (socket, addr) = listener.accept().await?;
        let stdin_ref = shared_stdin.clone();

        tokio::spawn(async move {
            let (read_half, write_half) = split(socket);
            let mut reader = BufReader::new(read_half);
            let mut buf = String::new();

            let (out_tx, mut out_rx) = mpsc::channel::<String>(32);
            let (cmd_tx, mut cmd_rx) = mpsc::channel::<String>(32);

                tokio::spawn(async move {
                    let mut writer = write_half;
                    loop {
                        tokio::select! {
                            Some(msg) = cmd_rx.recv() => {
                                let payload = json!({"type":"info","data":msg,"authcode": "0"}).to_string() + "\n";
                                let _ = writer.write_all(payload.as_bytes()).await;
                            }
                            Some(out) = out_rx.recv() => {
                                // println!("{}", out);
                                let _ = writer.write_all((out + "\n").as_bytes()).await;
                            }
                            else => break,
                        }
                    }
                    
                });

                loop {
                    buf.clear();
                    let n = reader.read_line(&mut buf).await;
                    if let Ok(0) = n { break; }
                    if let Err(e) = n { 
                        eprintln!("Read error: {}", e);
                    break; }

                    let line = buf.trim_end();
                    if line.starts_with('{') {
                            if let Ok(json_line) = serde_json::from_str::<Value>(&line) {
                                if let Ok(val) =  serde_json::from_value::<MessagePayload>(json_line.clone()) {
                                    let typ = val.r#type;
                                    if typ == "command" {
                                        let cmd_str = val.message;
                                        if cmd_str == "create_server" {
                                            if let Some(prov) = get_provider("minecraft") {
                                                if let Some(cmd) = prov.pre_hook() {
                                                    let _ = run_command_live_output(cmd, "Pre-hook".into(), Some(cmd_tx.clone()), None).await;
                                                }
                                                if let Some(cmd) = prov.install() {
                                                    let _ = run_command_live_output(cmd, "Install".into(), Some(cmd_tx.clone()), None).await;
                                                }
                                                if let Some(cmd) = prov.post_hook() {
                                                    let _ = run_command_live_output(cmd, "Post-hook".into(), Some(cmd_tx.clone()), None).await;
                                                }
                                                if let Some(cmd) = prov.start() {
                                                    println!("starting");
                                                    let tx = cmd_tx.clone();
                                                    let stdin_clone = stdin_ref.clone();
                                                    tokio::spawn(async move {
                                                        let _ = run_command_live_output(cmd, "Server".into(), Some(tx), Some(stdin_clone)).await;
                                                    });
                                                    let _ = cmd_tx.send("Server started".into()).await;
                                                }
                                            }
                                        } else if cmd_str == "start_server" { 
                                            if let Some(prov) = get_provider("minecraft") {
                                                if let Some(cmd) = prov.start() {
                                                    println!("starting");
                                                    let tx = cmd_tx.clone();
                                                    let stdin_clone = stdin_ref.clone();
                                                    tokio::spawn(async move {
                                                        let _ = run_command_live_output(cmd, "Server".into(), Some(tx), Some(stdin_clone)).await;
                                                    });
                                                    let _ = cmd_tx.send("Server started".into()).await;
                                                }
                                            }
                                        } else {
                                            let _ = cmd_tx.send(format!("Unknown command: {}", cmd_str)).await;
                                        }
                                    } else if typ == "console" {
                                        let input = val.message;
                                        let mut guard = stdin_ref.lock().await;
                                        if let Some(stdin) = guard.as_mut() {
                                            let _ = stdin.write_all(format!("{}\n", input).as_bytes()).await;
                                            let _ = stdin.flush().await;
                                            let _ = cmd_tx.send(format!("Sent to server: {}", input)).await;
                                        }
                                    }
                                } 
                                //if let Ok(val) = serde_json::from_value::<List>(json_line.clone()) {
                                // else if let Ok(val) = json_line.get("list").cloned().ok_or("fail").and_then(|item| List::try_from(item.get("data").cloned().unwrap())) as Result<List, _> {
                                //else if let Ok(val) = json_line.get("list").cloned().ok_or("fail").and_then(List::try_from) as Result<List, _> {
                                   // json_line.clone().try_into() as Result<List, _> {
                            else if let Ok(val) = json_line.clone().try_into() as Result<List, _> {
                                println!("Recived capabilities");
                                let _ = out_tx.send(
                                    serde_json::to_string(
                                        &List {
                                            list: vec!["all".to_string()]
                                        }
                                    )
                                    .unwrap()
                                ).await;
                            } else {
                                println!("{:#?}", json_line);
                            }
                        }
                } else if !line.is_empty() {
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
        });
    }
}
