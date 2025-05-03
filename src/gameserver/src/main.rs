use tokio::net::TcpListener;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

//
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let listener = TcpListener::bind("0.0.0.0:8080").await?;
    println!("TCP server listening on port 8080");

    loop {
        let (mut socket, addr) = listener.accept().await?;
        println!("New connection from {:?}", addr);

        tokio::spawn(async move {
            let mut buffer = [0; 1024];

            loop {
                match socket.read(&mut buffer).await {
                    Ok(0) => {
                        println!("Connection closed by {:?}", addr);
                        break;
                    }
                    Ok(n) => {
                        let received = String::from_utf8_lossy(&buffer[..n]).trim().to_string();
                        println!("Received from {}: {}", addr, received);

                        if received == "get_message" {
                            if let Err(e) = socket.write_all(b"Here is your message!\n").await {
                                eprintln!("Failed to write to socket: {:?}", e);
                                break;
                            }
                        } else {
                            // Optional: handle unrecognized messages
                            if let Err(e) = socket.write_all(b"Unrecognized command\n").await {
                                eprintln!("Failed to write to socket: {:?}", e);
                                break;
                            }
                        }
                    }
                    Err(e) => {
                        eprintln!("Failed to read from socket: {:?}", e);
                        break;
                    }
                }
            }
        });
    }
}
