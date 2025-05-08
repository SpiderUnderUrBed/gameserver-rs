use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader, split};
use serde_json::{Value, json};
use tokio::sync::{mpsc, Mutex};
use tokio::net::TcpListener;
use tokio::process::{Command as TokioCommand, ChildStdin};
use std::process::{Command, Stdio};
use std::sync::Arc;

const SERVER_DIR: &str = "server";

struct Minecraft;

trait Provider {
    fn pre_hook(&self) -> Option<Command>;
    fn install(&self) -> Option<Command>;
    fn post_hook(&self) -> Option<Command>;
    fn start(&self) -> Option<Command>;
    fn checks(&self) -> Option<Command>;
}

impl Provider for Minecraft {
    fn pre_hook(&self) -> Option<Command> {
        let mut cmd = Command::new("sh");
        cmd.arg("-c").arg("apt-get update && apt-get install -y libssl-dev pkg-config wget");
        Some(cmd)
    }

    fn install(&self) -> Option<Command> {
        let mut cmd = Command::new("sh");
        cmd.arg("-c").arg(
            "apt-get install -y openjdk-17-jre-headless && \
             update-alternatives --set java /usr/lib/jvm/java-17-openjdk-amd64/bin/java"
        );
        Some(cmd)
    }

    fn checks(&self) -> Option<Command> {
        let mut cmd = Command::new("java");
        cmd.arg("-version");
        Some(cmd)
    }

    fn post_hook(&self) -> Option<Command> {
        let mut cmd = Command::new("sh");
        cmd.arg("-c").arg(&format!(
            "mkdir -p {SERVER_DIR} && \
             cd {SERVER_DIR} && \
             wget -O server.jar https://piston-data.mojang.com/v1/objects/84194a2f286ef7c14ed7ce0090dba59902951553/server.jar && \
             echo 'eula=true' > eula.txt"
        ));
        Some(cmd)
    }

    fn start(&self) -> Option<Command> {
        let mut cmd = Command::new("java");
        cmd.arg("-Xmx1024M")
            .arg("-Xms1024M")
            .arg("-jar")
            .arg("server.jar")
            .arg("nogui")
            .current_dir(SERVER_DIR);
        Some(cmd)
    }
}
//
fn get_providier(provider: String) -> Option<impl Provider> {
    match provider.as_str() {
        "minecraft" => Some(Minecraft),
        _ => None,
    }
}

async fn run_command_live_output(
    mut cmd: Command,
    label: String,
    sender: Option<mpsc::Sender<String>>,
    stdin_arc: Option<Arc<Mutex<Option<ChildStdin>>>>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    println!("[DEBUG] Preparing to run command: {:?}", cmd);

    let mut tokio_cmd = TokioCommand::from(cmd);
    tokio_cmd.stdout(Stdio::piped()).stderr(Stdio::piped()).stdin(Stdio::piped());

    println!("[DEBUG] Spawning command...");
    let mut child = match tokio_cmd.spawn() {
        Ok(child) => {
            println!("[DEBUG] Command spawned successfully");
            child
        }
        Err(e) => {
            eprintln!("[ERROR] Failed to spawn command: {}", e);
            return Err(Box::new(e));
        }
    };

    if let Some(stdin) = stdin_arc {
        println!("[DEBUG] Setting up stdin...");
        let child_stdin = child.stdin.take();
        *stdin.lock().await = child_stdin;
        println!("[DEBUG] Stdin set");
    }

    let stdout = child.stdout.take();
    let stderr = child.stderr.take();

    let stdout_sender = sender.clone();
    let stderr_sender = sender;
    let stdout_label = label.clone();
    let stderr_label = label;

    println!("[DEBUG] Starting stdout and stderr handlers");

    let stdout_handle = tokio::spawn(async move {
        if let Some(stdout) = stdout {
            let mut reader = BufReader::new(stdout).lines();
            while let Ok(Some(line)) = reader.next_line().await {
                println!("[STDOUT DEBUG] {}", line);
                let output = json!({
                    "type": "stdout",
                    "data": format!("[{}] {}", stdout_label, line)
                });
                if let Some(sender) = &stdout_sender {
                    if let Err(e) = sender.send(output.to_string()).await {
                        eprintln!("[ERROR] Failed to send stdout line: {}", e);
                    }
                }
            }
            println!("[DEBUG] Stdout stream ended");
        } else {
            println!("[DEBUG] No stdout to read");
        }
    });

    let stderr_handle = tokio::spawn(async move {
        if let Some(stderr) = stderr {
            let mut reader = BufReader::new(stderr).lines();
            while let Ok(Some(line)) = reader.next_line().await {
                println!("[STDERR DEBUG] {}", line);
                let output = json!({
                    "type": "stderr",
                    "data": format!("[{}] {}", stderr_label, line)
                });
                if let Some(sender) = &stderr_sender {
                    if let Err(e) = sender.send(output.to_string()).await {
                        eprintln!("[ERROR] Failed to send stderr line: {}", e);
                    }
                }
            }
            println!("[DEBUG] Stderr stream ended");
        } else {
            println!("[DEBUG] No stderr to read");
        }
    });

    println!("[DEBUG] Waiting for child process to finish...");
    let status = child.wait().await?;
    println!("[DEBUG] Child process exited with: {}", status);

    let _ = stdout_handle.await;
    let _ = stderr_handle.await;

    println!("[DEBUG] Output handlers finished");

    Ok(())
}
//
//
//
//
//

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let listener = TcpListener::bind("0.0.0.0:8080").await?;
    println!("TCP server listening on port 8080");

    // Shared stdin for Minecraft process
    let minecraft_stdin = Arc::new(Mutex::new(None));

    loop {
        let (socket, addr) = listener.accept().await?;
        println!("New connection from {:?}", addr);
        let stdin_clone = minecraft_stdin.clone();

        tokio::spawn(async move {
            let (reader_half, writer_half) = split(socket);
            let mut reader = BufReader::new(reader_half);
            let mut line = String::new();

            let (output_tx, mut output_rx) = mpsc::channel::<String>(10);
            let (cmd_tx, mut cmd_rx) = mpsc::channel::<String>(10);

            let mut writer = writer_half;
            let writer_task = tokio::spawn(async move {
                loop {
                    tokio::select! {
                        Some(response) = cmd_rx.recv() => {
                            let msg = json!({
                                "type": "info",
                                "data": response.trim()
                            }).to_string() + "\n";
                            if writer.write_all(msg.as_bytes()).await.is_err() { break; }
                            if writer.flush().await.is_err() { break; }
                        }
                        Some(output) = output_rx.recv() => {
                            let msg = output + "\n";
                            if writer.write_all(msg.as_bytes()).await.is_err() { break; }
                            if writer.flush().await.is_err() { break; }
                        }
                        else => break,
                    }
                }
            });

            loop {
                line.clear();
                match reader.read_line(&mut line).await {
                    Ok(0) => {
                        println!("Connection closed by {:?}", addr);
                        break;
                    }
                    Ok(_) => {
                        let received = line.trim_end();
                        println!("Received from {}: {}", addr, received);

                        match serde_json::from_str::<Value>(received) {
                            Ok(message) => {
                                // let args = message["message"].split("")
                                // let final_message = message["message"];
                                if message["type"] == "command" {
                                    if message["message"] == "get_message" {
                                        let _ = cmd_tx.send("Here is your message!".into()).await;
                                    } else if message["message"] == "create_server" {
                                        println!("Creating Minecraft server...");
                                        let minecraft = get_providier("minecraft".to_string()).unwrap();
                                    
                                        if let Some(mut cmd) = minecraft.pre_hook() {
                                            println!("Running pre-hook...");
                                            if let Err(e) = run_command_live_output(cmd, "Pre-hook".into(), Some(cmd_tx.clone()), None).await {
                                                eprintln!("Pre-hook failed: {}", e);
                                            }
                                        } else {
                                            println!("No pre-hook command defined");
                                        }
                                    
                                        if let Some(mut cmd) = minecraft.install() {
                                            println!("Running install...");
                                            if let Err(e) = run_command_live_output(cmd, "Install".into(), Some(cmd_tx.clone()), None).await {
                                                eprintln!("Install failed: {}", e);
                                            }
                                        } else {
                                            println!("No install command defined");
                                        }
                                    
                                        if let Some(mut cmd) = minecraft.post_hook() {
                                            println!("Running post-hook...");
                                            if let Err(e) = run_command_live_output(cmd, "Post-hook".into(), Some(cmd_tx.clone()), None).await {
                                                eprintln!("Post-hook failed: {}", e);
                                            }
                                        } else {
                                            println!("No post-hook command defined");
                                        }
                                    
                                        if let Some(mut cmd) = minecraft.start() {
                                            println!("Starting Minecraft server...");
                                            if let Err(e) = run_command_live_output(cmd, "Server".into(), Some(cmd_tx.clone()), Some(stdin_clone.clone())).await {
                                                eprintln!("Server start failed: {}", e);
                                            }
                                        } else {
                                            println!("No start command defined");
                                        }
                                    } else {
                                        let _ = cmd_tx.send("Unrecognized command".into()).await;
                                    }
                                 } else if message["type"] == "console" {
                                    if let Some(input) = message["command"].as_str() {
                                        let mut stdin_guard = stdin_clone.lock().await;
                                        if let Some(stdin) = stdin_guard.as_mut() {
                                            if stdin.write_all(format!("{}\n", input).as_bytes()).await.is_ok() {
                                                let _ = cmd_tx.send(format!("Sent command to Minecraft: {}", input)).await;
                                            } else {
                                                let _ = cmd_tx.send("Failed to write to Minecraft stdin".into()).await;
                                            }
                                        } else {
                                            let _ = cmd_tx.send("Minecraft server is not running".into()).await;
                                        }
                                    }
                                }
                            }
                            Err(_) => {
                                let _ = cmd_tx.send("Invalid JSON format".into()).await;
                            }
                        }
                    }
                    Err(e) => {
                        eprintln!("Error reading line: {}", e);
                        break;
                    }
                }
            }

            let _ = writer_task.await;
        });
    }
}
