use std::{
    error::Error,
    fs::{self},
    net::SocketAddr,
};
use tower_http::services::ServeDir;

use axum::{
    routing::{get, get_service, post},
    Router,
    Json
};


// use kube::Error::Api;
use kube::Client;


use tokio::{
    io::AsyncWriteExt,
    net::{TcpListener, TcpStream},
    time::{sleep, Duration},
    sync::mpsc::{self, Sender, Receiver},
};


use serde_json::json;
use serde::{Serialize, Deserialize};

mod kubernetes;
mod docker;


#[derive(Serialize)]
struct MessagePayload {
    message: String,
}

async fn connect_to_server(mut rx: Receiver<Vec<u8>>) -> Result<(), Box<dyn Error>> {
    loop {
        match TcpStream::connect("gameserver-service:8080").await {
            Ok(mut stream) => {
                println!("Connected to server!");

                while let Some(data) = rx.recv().await {
                    if let Err(e) = stream.write_all(&data).await {
                        eprintln!("Failed to send data: {}", e);
                        break; 
                    }
                }

                break;
            }
            Err(e) => {
                println!("Failed to connect: {}. Retrying in 2 seconds...", e);
                sleep(Duration::from_secs(2)).await;
            }
        }
    }

    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    docker::build_docker_image().await?;
    let client = Client::try_default().await?;

    println!("Done with docker image");
    kubernetes::create_k8s_deployment(&client).await?;
    println!("Done with deployment");

    let (tx, rx): (Sender<Vec<u8>>, Receiver<Vec<u8>>) = mpsc::channel(32);

    tokio::spawn(async move {
        if let Err(e) = connect_to_server(rx).await {
            eprintln!("Connection task failed: {}", e);
        }
    });
    let payload = MessagePayload {
        message: "hello world!".to_string(),
    };
    let json_bytes = serde_json::to_vec(&payload)?;

    tx.send(json_bytes).await?;
    println!("Sent JSON message through the channel");

    // let message = String::from("This is a message from main");
    // tx.send(message.into_bytes()).await?;

    // println!("Sent a message through the channel");

    let app = Router::new()
        .route("/message", get(get_message))
        .route("/api/send", post(receive_message))
        .fallback_service(routes_static());

    let addr: SocketAddr = "127.0.0.1:8080".parse().unwrap();
    let listener = TcpListener::bind(addr).await.unwrap();
    println!("Listening on http://{}", addr);

    axum::serve(listener, app).await.unwrap();

    Ok(())
}


#[derive(Deserialize)]
struct IncomingMessage {
    message: String,
}

#[derive(Serialize)]
struct ResponseMessage {
    response: String,
}

async fn get_message() -> Json<serde_json::Value> {
    Json(json!({
        "message": "Hello from Axum!"
    }))
}


async fn receive_message(Json(payload): Json<IncomingMessage>) -> Json<ResponseMessage> {
    println!("Got message: {}", payload.message);
    Json(ResponseMessage {
        response: format!("You sent: {}", payload.message),
    })
}

fn routes_static() -> Router {
    Router::new().fallback_service(get_service(ServeDir::new("/usr/src/app/src/html/")).handle_error(|err| async move {
        (
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            format!("Unhandled internal error: {}", err),
        )
    }))
}


// fn routes() -> Router {
//     Router::new()
//         .route("/hello", get(handler))
// }

// async fn handler(Path(path): Path<String>) -> impl IntoResponse {
//     Html(format!("Hello, you are at <strong>{path}</strong>"))
// }

// let pods: Api<Pod> = Api::all(client.clone());
// let lp = ListParams::default();
// let pod_list = pods.list(&lp).await?;

// for p in pod_list.items {
//     if let Some(name) = p.metadata.name {
//         println!("Pod: {}", name);
//     }
// }
