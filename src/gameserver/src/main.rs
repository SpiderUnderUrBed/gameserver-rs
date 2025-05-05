use tokio::net::TcpListener;
use tokio::io::{AsyncReadExt, AsyncWriteExt, AsyncBufReadExt, BufReader, split};
use tokio::process::Command as TokioCommand;
use tokio::sync::mpsc;

use std::process::{Command, Stdio};
use serde_json::{Value, json};

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
        // These are system-level package installations
        let mut cmd = Command::new("sh");
        cmd.arg("-c").arg("apt-get update && apt-get install -y libssl-dev pkg-config wget");
        Some(cmd)
    }

    fn install(&self) -> Option<Command> {
        // System-level Java installation
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

// ... rest of your existing code remains the same ...

async fn run_command_live_output(
    cmd: Command,
    label: String,
    sender: Option<mpsc::Sender<String>>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let mut tokio_cmd = TokioCommand::from(cmd);
    tokio_cmd.stdout(Stdio::piped()).stderr(Stdio::piped());

    let mut child = tokio_cmd.spawn()?;

    let stdout = child.stdout.take();
    let stderr = child.stderr.take();

    let stdout_sender = sender.clone();
    let stderr_sender = sender;
    let stdout_label = label.clone();
    let stderr_label = label;

    let stdout_handle = tokio::spawn(async move {
        if let Some(stdout) = stdout {
            let mut reader = BufReader::new(stdout).lines();
            while let Ok(Some(line)) = reader.next_line().await {
                let output = json!({
                    "type": "stdout",
                    "data": format!("[{}] {}", stdout_label, line)
                });
                if let Some(sender) = &stdout_sender {
                    let _ = sender.send(output.to_string()).await;
                }
            }
        }
    });

    let stderr_handle = tokio::spawn(async move {
        if let Some(stderr) = stderr {
            let mut reader = BufReader::new(stderr).lines();
            while let Ok(Some(line)) = reader.next_line().await {
                let output = json!({
                    "type": "stderr",
                    "data": format!("[{}] {}", stderr_label, line)
                });
                if let Some(sender) = &stderr_sender {
                    let _ = sender.send(output.to_string()).await;
                }
            }
        }
    });

    let _ = child.wait().await?;
    let _ = stdout_handle.await;
    let _ = stderr_handle.await;

    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let listener = TcpListener::bind("0.0.0.0:8080").await?;
    println!("TCP server listening on port 8080");

    loop {
        let (socket, addr) = listener.accept().await?;
        println!("New connection from {:?}", addr);

        tokio::spawn(async move {
            let (mut reader, writer) = split(socket);
            let mut buffer = [0; 1024];

            let (output_tx, mut output_rx) = mpsc::channel::<String>(10);
            let (cmd_tx, mut cmd_rx) = mpsc::channel::<String>(10);

            let writer_task = tokio::spawn(async move {
                let mut writer = writer;
                loop {
                    tokio::select! {
                        Some(response) = cmd_rx.recv() => {
                            let message = json!({
                                "type": "info",
                                "data": response.trim()
                            });
                            if writer.write_all(message.to_string().as_bytes()).await.is_err() { break; }
                            if writer.write_all(b"\n").await.is_err() { break; }
                        }
                        Some(output) = output_rx.recv() => {
                            if writer.write_all(output.as_bytes()).await.is_err() { break; }
                            if writer.write_all(b"\n").await.is_err() { break; }
                        }
                        else => break,
                    }
                }
            });

            let provider = Minecraft;

            if let Some(cmd) = provider.checks() {
                if run_command_live_output(cmd, "Java Version Check".to_string(), Some(output_tx.clone())).await.is_err() {
                    let _ = cmd_tx.send("Failed to check Java version".to_string()).await;
                    let _ = writer_task.await;
                    return;
                }
            }

            loop {
                match reader.read(&mut buffer).await {
                    Ok(0) => {
                        println!("Connection closed by {:?}", addr);
                        break;
                    }
                    Ok(n) => {
                        let received = String::from_utf8_lossy(&buffer[..n]).trim().to_string();
                        println!("Received from {}: {}", addr, received);

                        let message = match serde_json::from_str::<Value>(&received) {
                            Ok(v) => v,
                            Err(_) => {
                                let _ = cmd_tx.send("Invalid JSON format".to_string()).await;
                                continue;
                            }
                        };

                        if message["message"] == "get_message" {
                            let _ = cmd_tx.send("Here is your message!".to_string()).await;
                        } else if message["message"] == "create_server" {
                            if let Some(cmd) = provider.pre_hook() {
                                if run_command_live_output(cmd, "Pre-hook".to_string(), None).await.is_err() {
                                    break;
                                }
                            }

                            if let Some(cmd) = provider.install() {
                                if run_command_live_output(cmd, "Install".to_string(), None).await.is_err() {
                                    break;
                                }
                            }

                            if let Some(cmd) = provider.post_hook() {
                                if run_command_live_output(cmd, "Post-hook".to_string(), None).await.is_err() {
                                    break;
                                }
                            }

                            if let Some(cmd) = provider.start() {
                                if run_command_live_output(cmd, "Start".to_string(), Some(cmd_tx.clone())).await.is_err() {
                                    break;
                                }
                            }
                        } else {
                            let _ = cmd_tx.send("Unrecognized command".to_string()).await;
                        }
                    }
                    Err(e) => {
                        eprintln!("Error reading from socket: {:?}", e);
                        break;
                    }
                }
            }

            let _ = writer_task.await;
        });
    }
}
