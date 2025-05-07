use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader, split};
use serde_json::{Value, json};
use tokio::sync::mpsc;
use tokio::net::TcpListener;
// …

use tokio::process::Command as TokioCommand;
use std::process::{Command, Stdio};

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

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let listener = TcpListener::bind("0.0.0.0:8080").await?;
    println!("TCP server listening on port 8080");

    loop {
        let (socket, addr) = listener.accept().await?;
        println!("New connection from {:?}", addr);

        tokio::spawn(async move {
            // Split into a reader half and a writer half
            let (reader_half, writer_half) = split(socket);
            let mut reader = BufReader::new(reader_half);
            let mut line = String::new();

            let (output_tx, mut output_rx) = mpsc::channel::<String>(10);
            let (cmd_tx, mut cmd_rx)     = mpsc::channel::<String>(10);

            // Writer task: send JSON + newline, then flush
            let mut writer = writer_half;
            let writer_task = tokio::spawn(async move {
                loop {
                    tokio::select! {
                        Some(response) = cmd_rx.recv() => {
                            let msg = json!({
                                "type": "info",
                                "data": response.trim()
                            })
                            .to_string() + "\n";
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

            // (Optional) run your initial checks, e.g. Java version...

            // Main read loop: read line by line
            loop {
                line.clear();
                match reader.read_line(&mut line).await {
                    Ok(0) => {
                        // client closed connection
                        println!("Connection closed by {:?}", addr);
                        break;
                    }
                    Ok(_) => {
                        let received = line.trim_end(); // no trailing newline
                        println!("Received from {}: {}", addr, received);

                        match serde_json::from_str::<Value>(received) {
                            Ok(message) => {
                                // handle your `get_message`, `create_server`, etc.
                                if message["message"] == "get_message" {
                                    let _ = cmd_tx.send("Here is your message!".into()).await;
                                }
                                // … other commands …
                                else {
                                    let _ = cmd_tx.send("Unrecognized command".into()).await;
                                    println!("Unrecognized command");
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

            // Wait for writer task to finish cleanly
            let _ = writer_task.await;
        });
    }
}
