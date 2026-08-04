use axum::extract::{State, ws::Utf8Bytes};
use general_networked_filesystem::{FileRequestExecutable, LsRequest};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    net::TcpStream,
    sync::{RwLock, broadcast},
    time::{sleep, timeout},
};
use tokio_util::sync::CancellationToken;

use crate::{ApiCalls as ToplevelApiCalls, AuthTcpMessage, IncomingMessage, List, NodeAndTCP};
use crate::{
    AppState, CHANNEL_BUFFER_SIZE, CONNECTION_RETRY_DELAY, CONNECTION_TIMEOUT, MessagePayload,
    MessagePayloadWithMetadata, MetadataTypes, SimpleMessage, SrcAndDest, Status, StreamResult,
    database::databasespec::Filters, handle_stream,
};
use anyhow::anyhow;
use std::{
    error::Error,
    net::SocketAddr,
    sync::{Arc, atomic::Ordering},
    time::{Duration, Instant},
};

pub struct PasswordRequest {
    pub password: String,
}
pub struct CapabilitiesRequest {
    pub capabilities: Vec<String>,
}
pub struct ServernameRequest {
    pub ip: String,
}
pub trait ImmediateTransportable {
    async fn immediate_transport(
        &self,
        state: &mut AppState,
    ) -> Result<(), Box<dyn Error + Send + Sync>>;
}

impl ImmediateTransportable for PasswordRequest {
    async fn immediate_transport(
        &self,
        state: &mut AppState,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        let auth_msg = serde_json::to_vec(&AuthTcpMessage {
            password: self.password.clone(),
        })?;
        let _ = state.connection_handler.proxy_tx.send(auth_msg);
        Ok(())
    }
}
impl ImmediateTransportable for CapabilitiesRequest {
    async fn immediate_transport(
        &self,
        state: &mut AppState,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        let capability_msg = serde_json::to_vec(&List {
            list: ToplevelApiCalls::Capabilities(self.capabilities.clone()),
        })?;
        let _ = state.connection_handler.proxy_tx.send(capability_msg);
        Ok(())
    }
}

impl ImmediateTransportable for ServernameRequest {
    async fn immediate_transport(
        &self,
        state: &mut AppState,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        let cmd_msg = serde_json::to_vec(&MessagePayload {
            r#type: "command".to_string(),
            message: "server_name".to_string(),
            authcode: "0".to_string(),
        })?;
        let _ = state.connection_handler.proxy_tx.send(cmd_msg);
        // writer.write_all(cmd_msg.as_bytes()).await?;

        'name: {
            // let mut state = arc_state.write().await;
            if let Ok(Ok(bytes)) = timeout(
                Duration::from_millis(1000),
                state.connection_handler.tcp_rx.recv(),
            )
            .await
            {
                if let Ok(payload) = serde_json::from_slice::<IncomingMessage>(&bytes) {
                    state.current_node = NodeAndTCP {
                        name: payload.message,
                        ip: self.ip.clone(),
                        ..Default::default()
                    };
                    break 'name;
                }
            }
            state.current_node = NodeAndTCP {
                name: "main".to_string(),
                ip: self.ip.clone(),
                ..Default::default()
            };
        }

        Ok(())
    }
}

// does the connection to the tcp server, wether initial or not, on success it will pass it off to the dedicated handler for the stream
// Changelog:
// No more blocking with stream, if the caller wants the server connection to not be blocking its the callers job to spawn it in
// its own thread
pub async fn connect_to_server(
    arc_state: Arc<RwLock<AppState>>,
    mut tcp_url: String,
    ws_tx: broadcast::Sender<String>,
    end_if_timeout: bool,
    block_with_stream: bool,
) -> Result<Option<SocketAddr>, Box<dyn Error + Send + Sync>> {
    let mut last_peer: Option<SocketAddr> = None;
    //let (proxy_tx, _) = broadcast::channel::<Vec<u8>>(CHANNEL_BUFFER_SIZE);
    // let (proxy_tx, _) = broadcast::channel::<Vec<u8>>(CHANNEL_BUFFER_SIZE);
    // let (_, proxy_rx) = broadcast::channel::<Vec<u8>>(CHANNEL_BUFFER_SIZE);
    let (proxy_tx, mut proxy_rx) = {
        let state_guard = arc_state.write().await;
        (
            state_guard.connection_handler.proxy_tx.clone(),
            state_guard.connection_handler.proxy_rx.resubscribe(),
        )
    };

    loop {
        let mut rx = {
            let state = arc_state.read().await;
            state.connection_handler.tcp_tx.subscribe()
        };
        let internal_stream = {
            let state = arc_state.read().await;
            state.internal_rx.as_ref().map(|r| r.resubscribe())
        };

        let deadline = Instant::now() + CONNECTION_TIMEOUT;
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return Err("overall connect timeout".into());
        }

        match timeout(remaining, TcpStream::connect(&tcp_url)).await {
            Ok(Ok(mut stream)) => {
                let peer = stream.peer_addr()?;
                last_peer = Some(peer);
                let cancel_token = CancellationToken::new();
                {
                    let mut state_guard = arc_state.write().await;
                    //state_guard.connection_handler.stream = Some(&stream);
                    state_guard.cancel_current_conn = cancel_token.clone();
                    state_guard.tcp_conn_status = Status::Up;
                }
                let ip = stream.peer_addr()?.ip().to_string();

                let (reader, mut writer) = stream.into_split();
                let buf_reader = BufReader::new(reader);
                let buf = vec![0u8; 4096];
                let mut lines = buf_reader.lines();

                let mut proxy_rx_clone = proxy_rx.resubscribe();
                let proxy_tx_clone = proxy_tx.clone();
                //let mut rx_clone = rx.resubscribe();

                //let arc_state_clone = arc_state.clone();

                tokio::spawn(async move {
                    loop {
                        tokio::select! {
                            _ = cancel_token.cancelled() => {
                                let _ = writer.shutdown();
                                break;
                            },
                            read_result = lines.next_line() => {
                                match read_result {
                                    Ok(Some(line)) => {
                                        println!("Got line: {:#?}", line.clone());
                                        let bytes = line.as_bytes();
                                        let _ = proxy_tx_clone.send(bytes.to_vec());
                                    },
                                    Ok(None) => {
                                        break;
                                    }
                                    Err(e) => {
                                        break;
                                    },
                                }
                            }
                            // proxy_rx_result = proxy_rx_clone.recv() => {
                            //     println!("got entry");
                            //     //println!("got bytes from entry {:#?}", String::from_utf8(bytes.clone()).unwrap());
                            //     //println!("{:#?}", proxy_rx_result);
                            //     if let Ok(bytes) = proxy_rx_result {
                            //         println!("returning bytes out");
                            //         if let Err(e) = writer.write_all(&bytes).await {

                            //         }
                            //         if let Err(e) = writer.write_all(b"\n").await {

                            //         };
                            //         if let Err(e) = writer.flush().await {

                            //         };
                            //     }
                            // }
                            rx = proxy_rx_clone.recv() => {
                                if let Ok(bytes) = rx {
                                    println!("got bytes to forward {:#?}", String::from_utf8(bytes.clone()).unwrap());
                                    //let _ = proxy_tx_clone.send(bytes.clone());
                                    //let mut state_guard = arc_state_clone.write().await;
                                    // let request_number = state_guard.connection_handler.request_number.load(Ordering::SeqCst);
                                    // println!("{:#?}", request_number);
                                    // *state_guard.connection_handler.request_number.get_mut() += 1;
                                    // drop(state_guard);
                                    // if let Err(e) = writer.write_all(&request_number.to_be_bytes()).await {
                                    //     println!("error");
                                    // }
                                    if let Err(e) = writer.write_all(&bytes).await {
                                        println!("error");
                                    }
                                    if let Err(e) = writer.write_all(b"\n").await {
                                        println!("error");
                                    };
                                    if let Err(e) = writer.flush().await {
                                        println!("error");
                                    };
                                }
                            }
                        }
                    }
                });

                let result = handle_stream(
                    Arc::clone(&arc_state),
                    &mut proxy_rx,
                    ip,
                    ws_tx.clone(),
                    internal_stream,
                )
                .await;

                // let result = if !block_with_stream {
                //     handle_stream(
                //         Arc::clone(&arc_state),
                //         &mut rx,
                //         ip,
                //         ws_tx.clone(),
                //         internal_stream,
                //     )
                //     .await
                // } else {
                //     handle_stream(
                //         Arc::clone(&arc_state),
                //         &mut rx,
                //         ip,
                //         ws_tx.clone(),
                //         internal_stream,
                //     )
                //     .await
                // };

                match result {
                    Ok(StreamResult::Reconnect(_, _)) => {}
                    // Ok(StreamResult::Reconnect(new_ip, new_name)) => {
                    //     println!("Reconnecting to {} ({})", new_name, new_ip);
                    //     {
                    //         let mut state = arc_state.write().await;
                    //         state.current_node = NodeAndTCP {
                    //             name: new_name,
                    //             ip: new_ip.clone(),
                    //             ..Default::default()
                    //         };
                    //         let node_state_bytes = serde_json::to_vec(&MessagePayload {
                    //             r#type: "command".to_string(),
                    //             message: "server_state".to_string(),
                    //             authcode: "0".to_string(),
                    //         })
                    //         .unwrap_or_default();
                    //         if let Some(tx) = &state.internal_tx {
                    //             let _ = tx.send(node_state_bytes);
                    //         }
                    //     }
                    //     tcp_url = new_ip;
                    //     continue;
                    // }
                    Ok(StreamResult::Done) => {
                        return Ok(Some(last_peer.unwrap_or(peer)));
                    }
                    Err(e) => {
                        eprintln!("handle_stream error: {}", e);
                        // Fall through to retry delay
                    }
                }
            }
            Ok(Err(e)) => {
                eprintln!("TCP connect error: {}", e);
                let mut state_guard = arc_state.write().await;
                if let Some(tx) = &state_guard.internal_tx {
                    let _ = tx.send("end_conn".into());
                }
                state_guard.tcp_conn_status = Status::Down;
            }
            Err(_) => {
                let mut state_guard = arc_state.write().await;
                state_guard.tcp_conn_status = Status::Down;
                eprintln!("TCP connect timed out");
                if end_if_timeout {
                    return Err("connection attempt timed out".into());
                }
            }
        }

        sleep(CONNECTION_RETRY_DELAY).await;
    }
}

pub async fn check_channel_health(
    state: &AppState,
    // tx: &broadcast::Sender<Vec<u8>>,
    // mut rx: broadcast::Receiver<Vec<u8>>,
) -> bool {
    let (tx, mut rx) = (
        state.connection_handler.proxy_tx.clone(),
        state.connection_handler.proxy_rx.resubscribe(),
    );
    match tx.send("ping".into()) {
        Ok(_) => true,
        Err(_) => return false,
    };

    match rx.recv().await {
        Ok(_msg) => true,
        Err(broadcast::error::RecvError::Closed) => false,
        Err(broadcast::error::RecvError::Lagged(_)) => true,
    }
}

// for the initial connection attempt, which will determine if possibly I would need to create the container and deployment upon failure
// i will use rusts 'timeout' for x interval determined with CONNECTION_TIMEOUT
async fn attempt_connection(
    tcp_url: String,
) -> Result<TcpStream, Box<dyn std::error::Error + Send + Sync>> {
    timeout(CONNECTION_TIMEOUT, TcpStream::connect(tcp_url))
        .await?
        .map_err(Into::into)
}

// this is where it determines wether or not to try and create the container and deployment, as attempt_connection itself is used in various diffrent contexts (like it will constantly
// try to connect upon failing but it should not try to create the container and deployment every time it fails)
// I use anyhow here because it saves me having to try and downcast the error type
pub(crate) async fn try_initial_connection(
    conn_attempts: u64,
    conn_timeout: u64,
    create_handler: bool,
    state: &Arc<RwLock<AppState>>,
    tcp_url: String,
    ws_tx: &broadcast::Sender<String>,
    tcp_tx: tokio::sync::broadcast::Sender<Vec<u8>>,
) -> Result<(), anyhow::Error> {
    let mut final_error = anyhow!(String::new());
    for _ in 0..conn_attempts {
        match attempt_connection(tcp_url.clone()).await {
            Ok(mut stream) => {
                println!("Initial connection succeeded!");
                // note, possibly I wont ever need to create a handler from the test of the intial connection
                // TODO: think about removing create_handler and just never create a handler here
                // I was considering to return the handler from here, but it wouldnt make sense to add that complexity
                // when I only create the initial tcp stream within the main function, it would involve either a thread here, or in the main function
                // and i rather keep this function focused on testing the connection (there might be a very NICHE case for making a handler here, but if there isnt ill remove it)
                if create_handler {
                    let (_, temp_rx) =
                        tokio::sync::broadcast::channel::<Vec<u8>>(CHANNEL_BUFFER_SIZE);
                    let mut temp_rx = temp_rx;
                    let ip: String = stream.peer_addr()?.ip().to_string();

                    let stream_result =
                        handle_stream(state.clone(), &mut temp_rx, ip, ws_tx.clone(), None).await;
                    if stream_result.is_ok() {
                        println!("Stream finished");
                        return Ok(());
                    } else {
                        final_error = anyhow!(stream_result.err().unwrap())
                    }
                } else {
                    return Ok(());
                }
            }
            Err(e) => {
                eprintln!("Initial connection failed: {}", e);
            }
        }
        tokio::time::sleep(Duration::from_secs(2)).await;
    }
    Err(final_error)
}

pub trait NodeTransportable {
    async fn node_transport(&self, state: &AppState) -> Result<(), Box<dyn Error + Send + Sync>>;
}
impl NodeTransportable for LsRequest {
    async fn node_transport(&self, state: &AppState) -> Result<(), Box<dyn Error + Send + Sync>> {
        let mut bytes = Vec::new();
        bytes.push(LsRequest::item_id());
        bytes.push(self.id);
        match serde_json::to_vec(&self) {
            Ok(b) => bytes.extend(b),
            Err(e) => {
                eprintln!("Serialization error: {}", e);
                return Err("Failed to serialize".into());
            }
        };
        let _ = state.connection_handler.proxy_tx.send(bytes);
        Ok(())
    }
}

pub struct DeleteServerRequest {
    pub metadata: MetadataTypes,
}
// NodeTransportable
impl NodeTransportable for DeleteServerRequest {
    async fn node_transport(&self, state: &AppState) -> Result<(), Box<dyn Error + Send + Sync>> {
        let msg = MessagePayloadWithMetadata {
            r#type: "command".to_string(),
            message: "delete_server".to_string(),
            authcode: "0".to_string(),
            metadata: self.metadata.clone(),
        };

        let mut bytes = match serde_json::to_vec(&msg) {
            Ok(b) => b,
            Err(e) => {
                eprintln!("Serialization error: {}", e);
                return Err("Failed to serialize".into());
            }
        };
        bytes.push(b'\n');

        let _ = state.connection_handler.proxy_tx.send(bytes);

        Ok(())
    }
}

pub struct ConnectionHandler {
    //stream: Option<&'static TcpStream>,
    pub(crate) proxy_tx: tokio::sync::broadcast::Sender<Vec<u8>>,
    pub(crate) proxy_rx: tokio::sync::broadcast::Receiver<Vec<u8>>,
    pub(crate) tcp_tx: tokio::sync::broadcast::Sender<Vec<u8>>,
    pub(crate) tcp_rx: tokio::sync::broadcast::Receiver<Vec<u8>>,
}
impl ConnectionHandler {
    pub fn new() -> Self {
        let (tcp_tx, tcp_rx) = broadcast::channel::<Vec<u8>>(CHANNEL_BUFFER_SIZE);
        let (proxy_tx, proxy_rx) = broadcast::channel::<Vec<u8>>(CHANNEL_BUFFER_SIZE);
        ConnectionHandler {
            //stream: None,
            proxy_tx,
            proxy_rx,
            tcp_tx,
            tcp_rx,
        }
    }
    pub fn get_filesystem_stream(
        &self,
    ) -> (broadcast::Sender<Vec<u8>>, broadcast::Receiver<Vec<u8>>) {
        (self.proxy_tx.clone(), self.proxy_rx.resubscribe())
    }
}
impl Default for ConnectionHandler {
    fn default() -> Self {
        let (tcp_tx, tcp_rx) = broadcast::channel::<Vec<u8>>(CHANNEL_BUFFER_SIZE);
        let (proxy_tx, proxy_rx) = broadcast::channel::<Vec<u8>>(CHANNEL_BUFFER_SIZE);
        ConnectionHandler {
            //stream: None,
            proxy_tx,
            proxy_rx,
            tcp_tx,
            tcp_rx,
        }
    }
}
impl Clone for ConnectionHandler {
    fn clone(&self) -> Self {
        ConnectionHandler {
            //stream: None,
            proxy_tx: self.proxy_tx.clone(),
            proxy_rx: self.proxy_rx.resubscribe(),
            tcp_tx: self.tcp_tx.clone(),
            tcp_rx: self.tcp_rx.resubscribe(),
        }
    }
}
// impl ConnectionHandler {
//     pub async fn shutdown(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>>{
//         Ok(())
//     }
// }

pub struct CreateServerRequest {
    pub metadata: MetadataTypes,
}
impl NodeTransportable for CreateServerRequest {
    async fn node_transport(&self, state: &AppState) -> Result<(), Box<dyn Error + Send + Sync>> {
        let msg = MessagePayloadWithMetadata {
            r#type: "command".to_string(),
            message: "create_server".to_string(),
            metadata: self.metadata.clone(),
            authcode: "0".to_string(),
        };
        let mut bytes = match serde_json::to_vec(&msg) {
            Ok(b) => b,
            Err(e) => {
                eprintln!("Serialization error: {}", e);
                return Err("Failed to serialize".into());
            }
        };
        bytes.push(b'\n');
        let _ = state.connection_handler.proxy_tx.send(bytes);

        Ok(())
    }
}
// NodeTransportable

pub struct StartServerRequest {
    // metadata: MetadataTypes
}
impl NodeTransportable for StartServerRequest {
    async fn node_transport(&self, state: &AppState) -> Result<(), Box<dyn Error + Send + Sync>> {
        let msg = serde_json::to_vec(&MessagePayload {
            r#type: "command".to_string(),
            message: "start_server".to_string(),
            authcode: "".to_string(),
        });
        if let Err(e) = msg {
            return Err("Failed to serialize".into());
        };
        let _ = state.connection_handler.proxy_tx.send(msg.unwrap());

        Ok(())
    }
}



pub struct StopServerRequest {
    // metadata: MetadataTypes
}
impl NodeTransportable for StopServerRequest {
    async fn node_transport(&self, state: &AppState) -> Result<(), Box<dyn Error + Send + Sync>> {
        let msg = serde_json::to_vec(&MessagePayload {
            r#type: "command".to_string(),
            message: "stop_server".to_string(),
            authcode: "".to_string(),
        });
        if let Err(e) = msg {
            return Err("Failed to serialize".into());
        };
        let _ = state.connection_handler.proxy_tx.send(msg.unwrap());

        Ok(())
    }
}

#[derive(Serialize, Deserialize)]
pub struct MigrateRequest {
    #[serde(flatten)]
    pub common: SrcAndDest,
}
impl NodeTransportable for MigrateRequest {
    async fn node_transport(&self, state: &AppState) -> Result<(), Box<dyn Error + Send + Sync>> {
        match serde_json::to_vec(&self.common) {
            Ok(bytes) => {
                if let Err(err) = state.connection_handler.proxy_tx.send(bytes) {
                    eprintln!("Failed to send request over broadcast: {}", err);
                }
            }
            Err(err) => eprintln!("Failed to serialize request: {}", err),
        }

        Ok(())
    }
}

pub struct SetServerRequest {
    pub(crate) metadata: MetadataTypes,
}
impl NodeTransportable for SetServerRequest {
    async fn node_transport(&self, state: &AppState) -> Result<(), Box<dyn Error + Send + Sync>> {
        let msg = MessagePayloadWithMetadata {
            r#type: "command".to_string(),
            message: "set_server".to_string(),
            metadata: self.metadata.clone(),
            authcode: "0".to_string(),
        };
        let mut bytes = match serde_json::to_vec(&msg) {
            Ok(b) => b,
            Err(e) => {
                eprintln!("Serialization error: {}", e);
                return Err("Failed to serialize".into());
            }
        };
        bytes.push(b'\n');
        let _ = state.connection_handler.proxy_tx.send(bytes);

        Ok(())
    }
}
// NodeTransportable

pub struct ServerDataRequest {
    pub(crate) metadata: MetadataTypes,
}
impl NodeTransportable for ServerDataRequest {
    async fn node_transport(&self, state: &AppState) -> Result<(), Box<dyn Error + Send + Sync>> {
        let msg = MessagePayloadWithMetadata {
            r#type: "command".to_string(),
            message: "server_data".to_string(),
            metadata: self.metadata.clone(),
            authcode: "0".to_string(),
        };
        let mut bytes = match serde_json::to_vec(&msg) {
            Ok(b) => b,
            Err(e) => {
                eprintln!("Serialization error: {}", e);
                return Err("Failed to serialize".into());
            }
        };
        bytes.push(b'\n');
        let _ = state.connection_handler.proxy_tx.send(bytes);

        Ok(())
    }
}

// struct ServerState
// NodeTransportable

// pub struct RawBytes {
//     pub(crate) bytes: Vec<u8>,
// }

// impl NodeTransportable for RawBytes {
//     async fn node_transport(&self, state: &AppState) -> Result<(), Box<dyn Error + Send + Sync>> {
//         let _ = state.connection_handler.proxy_tx.send(self.bytes.clone());
//         Ok(())
//     }
// }
// NodeTransportable

trait InternalTransportable {
    async fn internal_transport(
        &self,
        state: &AppState,
    ) -> Result<(), Box<dyn Error + Send + Sync>>;
}
pub struct FilterRequest {
    //pub(crate) //metadata: MetadataTypes
    pub(crate) filter: Filters,
}
//InternalTransportable
// struct FilterRequest impl NodeTransportable {
impl NodeTransportable for FilterRequest {
    async fn node_transport(&self, state: &AppState) -> Result<(), Box<dyn Error + Send + Sync>> {
        let filter_request = MessagePayloadWithMetadata {
            r#type: "command".to_string(),
            message: "set_filter".to_string(),
            metadata: MetadataTypes::Filter(self.filter.clone()),
            authcode: "0".to_string(),
        };
        let _ = state
            .connection_handler
            .tcp_tx
            .send(serde_json::to_vec(&filter_request).unwrap());

        Ok(())
    }
}
impl InternalTransportable for FilterRequest {
    async fn internal_transport(
        &self,
        state: &AppState,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        Ok(())
    }
}

// TODO: add and associated type and generic for Ok type while the errors be an eum
trait ResultTransportable {
    async fn transport_and_recv(
        &self,
        state: &AppState,
    ) -> Result<(), Box<dyn Error + Send + Sync>>;
}
// }
pub struct Ping {}
impl NodeTransportable for Ping {
    async fn node_transport(&self, state: &AppState) -> Result<(), Box<dyn Error + Send + Sync>> {
        let ping = SimpleMessage {
            message: "ping".to_string(),
        };
        let res = state
            .connection_handler
            .proxy_tx
            .send(serde_json::to_vec(&ping).unwrap());
        Ok(())
    }
}
impl ResultTransportable for Ping {
    async fn transport_and_recv(
        &self,
        state: &AppState,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        Ok(())
    }
}
impl InternalTransportable for Ping {
    async fn internal_transport(
        &self,
        state: &AppState,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        Ok(())
    }
}
//InternalTransportable
pub struct IntegrationKeyRequest {
    pub key: Value,
}
impl NodeTransportable for IntegrationKeyRequest {
    async fn node_transport(&self, state: &AppState) -> Result<(), Box<dyn Error + Send + Sync>> {
        match serde_json::to_vec(&self.key) {
            Ok(mut bytes) => {
                // Add newline delimiter for TCP stream parsing
                bytes.push(b'\n');

                if let Err(err) = state.connection_handler.proxy_tx.send(bytes.clone()) {
                    eprintln!("Failed to send to internal stream: {}", err);
                }

                // Tells the remote server to enable RCON
                //if let Some(internal_tx) = &state.internal_tx {
                if let Err(err) = state.connection_handler.proxy_tx.send(bytes) {
                    eprintln!("Failed to send to TCP stream: {}", err);
                }
                //}
            }
            Err(err) => eprintln!("Failed to serialize request: {}", err),
        }

        Ok(())
    }
}
impl InternalTransportable for IntegrationKeyRequest {
    async fn internal_transport(
        &self,
        state: &AppState,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        Ok(())
    }
}
//InternalTransportable
pub struct ServerStateRequest {}
impl NodeTransportable for ServerStateRequest {
    async fn node_transport(&self, state: &AppState) -> Result<(), Box<dyn Error + Send + Sync>> {
        let msg = serde_json::to_vec(&MessagePayload {
            r#type: "command".to_string(),
            message: "server_state".to_string(),
            authcode: "0".to_string(),
        })
        .unwrap();
        let _ = state.connection_handler.proxy_tx.send(msg);

        Ok(())
    }
}
impl InternalTransportable for ServerStateRequest {
    async fn internal_transport(
        &self,
        state: &AppState,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        Ok(())
    }
}

// NoteTransportable
//InternalTransportable
// struct SrcAndDestFsRequest {
//     src: String,
//     dest: String
// }
//InternalTransportable
// async fn send_request_to_node(){
// }
