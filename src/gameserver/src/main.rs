use tokio::net::TcpListener;
use tokio::io::{AsyncReadExt, AsyncWriteExt, AsyncBufReadExt, BufReader, split};
use tokio::process::Command as TokioCommand;
use tokio::sync::mpsc;

use std::process::{Command, Stdio};
use serde_json::{Value, json};

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
        cmd.arg("-c")
            .arg("apt-get update && apt-get install -y libssl-dev pkg-config wget");
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
        cmd.arg("-c").arg(
            "mkdir -p server && \
             wget -O server/server.jar https://piston-data.mojang.com/v1/objects/84194a2f286ef7c14ed7ce0090dba59902951553/server.jar && \
             echo 'eula=true' > server/eula.txt",
        );
        Some(cmd)
    }

    fn start(&self) -> Option<Command> {
        let mut cmd = Command::new("java");
        cmd.arg("-Xmx1024M")
            .arg("-Xms1024M")
            .arg("-jar")
            .arg("server.jar")
            .arg("nogui")
            .current_dir("server");
        Some(cmd)
    }
}

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

    // Clone the sender and label for each handler
    let stdout_sender = sender.clone();
    let stderr_sender = sender;
    let stdout_label = label.clone();
    let stderr_label = label;

    let stdout_handle = tokio::spawn(async move {
        if let Some(stdout) = stdout {
            let mut reader = BufReader::new(stdout).lines();
            while let Ok(Some(line)) = reader.next_line().await {
                let output = format!("[{} stdout] {}", stdout_label, line);
                println!("{}", output);
                if let Some(sender) = &stdout_sender {
                    let _ = sender.send(output).await;
                }
            }
        }
    });

    let stderr_handle = tokio::spawn(async move {
        if let Some(stderr) = stderr {
            let mut reader = BufReader::new(stderr).lines();
            while let Ok(Some(line)) = reader.next_line().await {
                let output = format!("[{} stderr] {}", stderr_label, line);
                eprintln!("{}", output);
                if let Some(sender) = &stderr_sender {
                    let _ = sender.send(output).await;
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

            // Create channels
            let (cmd_tx, mut cmd_rx) = mpsc::channel::<String>(10);

            // Writer task
            let writer_task = tokio::spawn(async move {
                let mut writer = writer;
                loop {
                    tokio::select! {
                        Some(response) = cmd_rx.recv() => {
                            if let Err(e) = writer.write_all(response.as_bytes()).await {
                                eprintln!("Failed to write command response: {:?}", e);
                                break;
                            }
                        }
                        Some(output) = output_rx.recv() => {
                            let message = json!({
                                "type": "output",
                                "data": output
                            });
                            if let Err(e) = writer.write_all(message.to_string().as_bytes()).await {
                                eprintln!("Failed to write output: {:?}", e);
                                break;
                            }
                            if let Err(e) = writer.write_all(b"\n").await {
                                eprintln!("Failed to write newline: {:?}", e);
                                break;
                            }
                        }
                        else => break,
                    }
                }
            });

            let provider = Minecraft;

            // Check Java version first
            if let Some(cmd) = provider.checks() {
                if let Err(_e) = run_command_live_output(cmd, "Java Version Check".to_string(), Some(output_tx.clone())).await {
                    let _ = cmd_tx.send("Failed to check Java version\n".to_string()).await;
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
                            Err(e) => {
                                eprintln!("Failed to parse JSON: {}", e);
                                let _ = cmd_tx.send("Invalid JSON format\n".to_string()).await;
                                continue;
                            }
                        };

                        if message["message"] == "get_message" {
                            let _ = cmd_tx.send("Here is your message!\n".to_string()).await;
                        } else if message["message"] == "create_server" {
                            // Run commands sequentially
                            if let Some(cmd) = provider.pre_hook() {
                                if let Err(_e) = run_command_live_output(cmd, "Pre-hook".to_string(), Some(cmd_tx.clone())).await {
                                    eprintln!("Pre-hook failed");
                                    break;
                                }
                            }

                            if let Some(cmd) = provider.install() {
                                if let Err(_e) = run_command_live_output(cmd, "Install".to_string(), Some(cmd_tx.clone())).await {
                                    eprintln!("Install failed");
                                    break;
                                }
                            }

                            if let Some(cmd) = provider.post_hook() {
                                if let Err(_e) = run_command_live_output(cmd, "Post-hook".to_string(), Some(cmd_tx.clone())).await {
                                    eprintln!("Post-hook failed");
                                    break;
                                }
                            }
                            //
                            if let Some(cmd) = provider.start() {
                                if let Err(_e) = run_command_live_output(cmd, "Start".to_string(), Some(cmd_tx.clone())).await {
                                    eprintln!("Start failed");
                                    break;
                                }
                            }
                        } else {
                            let _ = cmd_tx.send("Unrecognized command\n".to_string()).await;
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