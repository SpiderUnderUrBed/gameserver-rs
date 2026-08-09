use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::{
    net::{unix::pipe::Receiver, TcpStream},
    sync::{broadcast, mpsc, Mutex, RwLock},
    time::{sleep, timeout},
};
use tokio_stream::wrappers::ReceiverStream;
use tokio_stream::StreamExt;
use crate::{transport::node_transport::proto::ServerMessage, ConsoleData, CHANNEL_BUFFER_SIZE};
use crate::{ApiCalls as ToplevelApiCalls, AuthTcpMessage, IncomingMessage, List, NodeAndTCP};
use crate::{
    AppState, CONNECTION_RETRY_DELAY, CONNECTION_TIMEOUT, MessagePayload,
    MessagePayloadWithMetadata, MetadataTypes, SimpleMessage, SrcAndDest, Status, StreamResult,
    database::databasespec::Filters, handle_stream,
};
use anyhow::anyhow;
use std::time::Duration;
use std::{error::Error, net::SocketAddr, sync::Arc, time::Instant};

use tonic::transport::Channel;
mod proto {
    tonic::include_proto!("main");
}
use proto::{
    DeleteServerRequest as DeleteServerRequestGrpc, filesystem_client::FilesystemClient,
    general_client::GeneralClient, server_edit_client::ServerEditClient,
    server_edit_server::ServerEdit, server_manage_client::ServerManageClient,
};

#[derive(Clone)]
pub struct Clients {
    general_client: GeneralClient<Channel>,
    server_manage_client: ServerManageClient<Channel>,
    server_edit_client: ServerEditClient<Channel>,
    filesystem_client: FilesystemClient<Channel>,
}

pub struct ConnectionHandler {
    //stream: Option<&'static TcpStream>,
    clients: Option<Clients>,
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
            clients: None,
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
            clients: None,
        }
    }
}
impl Clone for ConnectionHandler {
    fn clone(&self) -> Self {
        ConnectionHandler {
            //stream: None,
            clients: self.clients.clone(),
            proxy_tx: self.proxy_tx.clone(),
            proxy_rx: self.proxy_rx.resubscribe(),
            tcp_tx: self.tcp_tx.clone(),
            tcp_rx: self.tcp_rx.resubscribe(),
        }
    }
}
pub async fn check_channel_health(
    state: &AppState,
) -> bool {
    true
}

// does the connection to the tcp server, wether initial or not, on success it will pass it off to the dedicated handler for the stream
pub async fn connect_to_server(
    arc_state: Arc<RwLock<AppState>>,
    url: String,
    _ws_tx: broadcast::Sender<String>,
    _end_if_timeout: bool,
    _block_with_stream: bool,
) -> Result<Option<SocketAddr>, Box<dyn Error + Send + Sync>> {
    println!("using this connect to server");
    let mut state = arc_state.write().await;

    let url = if url.starts_with("http://") || url.starts_with("https://") {
        url
    } else {
        format!("http://{url}")
    };

    let channel = Channel::from_shared(url)?.connect().await?;
    let general_client = GeneralClient::new(channel.clone());
    let filesystem_client = FilesystemClient::new(channel.clone());
    let server_edit_client = ServerEditClient::new(channel.clone());
    let server_manage_client = ServerManageClient::new(channel.clone());

    state.connection_handler.clients = Some(Clients {
        general_client,
        server_manage_client,
        server_edit_client,
        filesystem_client,
    });

    Ok(None)
}


// this is where it determines wether or not to try and create the container and deployment, as attempt_connection itself is used in various diffrent contexts (like it will constantly
// try to connect upon failing but it should not try to create the container and deployment every time it fails)
// I use anyhow here because it saves me having to try and downcast the error type
pub async fn try_initial_connection(
    _conn_attempts: u64,
    _conn_timeout: u64,
    _create_handler: bool,
    _state: &Arc<RwLock<AppState>>,
    _tcp_url: String,
    _ws_tx: &broadcast::Sender<String>,
    _tcp_tx: tokio::sync::broadcast::Sender<Vec<u8>>,
) -> Result<(), anyhow::Error> {
    Ok(())
}

pub trait NodeTransportable {
    type Output;
    async fn node_transport(
        &self,
        state: &mut AppState,
    ) -> Result<Self::Output, Box<dyn Error + Send + Sync>>;
}

// TODO: consider if needed
pub trait ImmediateTransportable {
    async fn immediate_transport(
        &self,
        state: &mut AppState,
    ) -> Result<(), Box<dyn Error + Send + Sync>>;
}

pub struct PasswordRequest {
    pub password: String,
}
pub struct CapabilitiesRequest {
    pub capabilities: Vec<String>,
}
pub struct ServernameRequest {
    pub ip: String,
}
impl ImmediateTransportable for PasswordRequest {
    async fn immediate_transport(
        &self,
        state: &mut AppState,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        let auth_msg = serde_json::to_vec(&AuthTcpMessage {
            password: self.password.clone(),
        })?;
        let _ = state.connection_handler.tcp_tx.send(auth_msg);
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
        let _ = state.connection_handler.tcp_tx.send(capability_msg);
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

pub struct DeleteServerRequest {
    pub metadata: MetadataTypes,
}

// NodeTransportable
impl NodeTransportable for DeleteServerRequest {
    type Output = ();
    async fn node_transport(
        &self,
        state: &mut AppState,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        let request = proto::DeleteServerRequest {
            metadata: Some(self.metadata.clone().into()),
        };
        let _ = state
            .connection_handler
            .clients
            .as_mut()
            .unwrap()
            .server_edit_client
            .delete(request);

        Ok(())
    }
}

pub struct CreateServerRequest {
    pub metadata: MetadataTypes,
}
impl NodeTransportable for CreateServerRequest {
    type Output = ();
    async fn node_transport(
        &self,
        state: &mut AppState,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        let request = proto::CreateServerRequest {
            metadata: Some(self.metadata.clone().into()),
        };
        let _ = state
            .connection_handler
            .clients
            .as_mut()
            .unwrap()
            .server_edit_client
            .create(request);

        Ok(())
    }
}
impl StreamTransportable for CreateServerRequest {
    type Output = mpsc::Receiver<ConsoleData>;
    async fn stream_transport(
        &self,
        state: Arc<RwLock<AppState>>,
    ) -> Result<Self::Output, Box<dyn Error + Send + Sync>> {
        let request = proto::CreateServerRequest {
            metadata: Some(self.metadata.clone().into())
        };
        let (server_out_tx, server_out_rx) = tokio::sync::mpsc::channel(32);
        let mut clients = {
            let guard = state.read().await;
            guard.connection_handler.clients.clone().unwrap()
        }; 

        if let Ok(response_stream) = clients.server_edit_client.create(request).await {
            {
                //drop(state);
                println!("before creating stream");
                let mut stream = response_stream.into_inner();
                tokio::spawn(async move {
                    while let Some(result) = stream.next().await {
                        match result {
                            Ok(message) => {
                                //println!("got a message {:#?}", message);
                                let _ = server_out_tx.send(
                                    ConsoleData { authcode: "0".to_string(), data: message.data, r#type: message.r#type }
                                ).await;
                            },
                            Err(e) => {
                            }
                        }
                    }
                });
            } 
        Ok(server_out_rx)
    } else {
        Err("error".into())
    }
}
}

//
pub trait StreamTransportable {
    type Output;
    async fn stream_transport(
        &self,
        state: Arc<RwLock<AppState>>,
    ) -> Result<Self::Output, Box<dyn Error + Send + Sync>>;
}
pub struct StartServerRequest {
    // metadata: MetadataTypes
    pub stdin: Option<broadcast::Receiver<String>>
}
impl StreamTransportable for StartServerRequest {
    type Output = mpsc::Receiver<ConsoleData>;
    async fn stream_transport(
        &self,
        state: Arc<RwLock<AppState>>,
    ) -> Result<Self::Output, Box<dyn Error + Send + Sync>> {
        let request = proto::StartServerRequest {};
        let (server_out_tx, server_out_rx) = tokio::sync::mpsc::channel(32);
        let (server_in_tx, server_in_rx) = tokio::sync::mpsc::channel(32);
        let outbound_stream = ReceiverStream::new(server_in_rx);
        if let Some(mut stdin) = self.stdin.as_ref().map(|r| r.resubscribe()) {
            tokio::spawn(async move {
                loop {
                    if let Ok(data) = stdin.recv().await {
                        let _ = server_in_tx.send(
                            ServerMessage { 
                                authcode: "0".to_string(), 
                                data, 
                                r#type: "console".to_string()
                            }
                        ).await;
                    } else {
                        break;
                    }
                }
            });
        }
        let mut clients = {
            let guard = state.read().await;
            guard.connection_handler.clients.clone().unwrap()
        }; 

        if let Ok(response_stream) = clients.server_edit_client.start(outbound_stream).await {
            {
                //drop(state);
                println!("before starting stream");
                let mut stream = response_stream.into_inner();
                tokio::spawn(async move {
                    while let Some(result) = stream.next().await {
                        match result {
                            Ok(message) => {
                                //println!("got a message {:#?}", message);
                                let _ = server_out_tx.send(
                                    ConsoleData { authcode: "0".to_string(), data: message.data, r#type: message.r#type }
                                ).await;
                            },
                            Err(e) => {
                                println!("got an err");
                            }
                        }
                    }
                });
            } 
        Ok(server_out_rx)
    } else {
        Err("error".into())
    }
}
}

pub struct StopServerRequest {
    // metadata: MetadataTypes
}
impl NodeTransportable for StopServerRequest {
    type Output = ();
    async fn node_transport(
        &self,
        state: &mut AppState,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        let stop_server_request = proto::StopServerRequest {};
        let _ = state
            .connection_handler
            .clients
            .clone()
            .unwrap()
            .server_edit_client
            .stop(stop_server_request)
            .await;

        Ok(())
    }
}

#[derive(Serialize, Deserialize)]
pub struct MigrateRequest {
    #[serde(flatten)]
    pub common: SrcAndDest,
}
impl NodeTransportable for MigrateRequest {
    type Output = ();
    async fn node_transport(
        &self,
        state: &mut AppState,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        match serde_json::to_vec(&self.common) {
            Ok(bytes) => {
                if let Err(err) = state.connection_handler.tcp_tx.send(bytes) {
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
    type Output = ();
    async fn node_transport(
        &self,
        state: &mut AppState,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        let set_server_request = proto::SetServerRequest {
            message: "set_server".to_string(),
            r#type: "command".to_string(),
            metadata: Some(self.metadata.clone().into()),
            authcode: "0".to_string(),
        };
        let _ = state
            .connection_handler
            .clients
            .as_mut()
            .unwrap()
            .server_manage_client
            .set(set_server_request);

        Ok(())
    }
}
// NodeTransportable

pub struct ServerDataRequest {
    pub(crate) metadata: MetadataTypes,
}
impl NodeTransportable for ServerDataRequest {
    type Output = ();
    async fn node_transport(
        &self,
        state: &mut AppState,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        let server_data_request = proto::ServerDataRequest {};
        let _ = state
            .connection_handler
            .clients
            .as_mut()
            .unwrap()
            .server_manage_client
            .data(server_data_request);

        Ok(())
    }
}

pub struct RawBytes {
    pub(crate) bytes: Vec<u8>,
}

impl NodeTransportable for RawBytes {
    type Output = ();
    async fn node_transport(
        &self,
        state: &mut AppState,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        let _ = state.connection_handler.tcp_tx.send(self.bytes.clone());
        Ok(())
    }
}

trait InternalTransportable {
    async fn internal_transport(
        &self,
        state: &AppState,
    ) -> Result<(), Box<dyn Error + Send + Sync>>;
}
pub struct FilterRequest {
    pub(crate) filter: Filters,
}
//InternalTransportable
impl NodeTransportable for FilterRequest {
    type Output = ();
    async fn node_transport(
        &self,
        state: &mut AppState,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
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

// }
pub struct Ping {}
impl NodeTransportable for Ping {
    type Output = ();
    async fn node_transport(
        &self,
        state: &mut AppState,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        let ping = SimpleMessage {
            message: "ping".to_string(),
        };
        let res = state
            .connection_handler
            .tcp_tx
            .send(serde_json::to_vec(&ping).unwrap());

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
    type Output = ();
    async fn node_transport(
        &self,
        state: &mut AppState,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        match serde_json::to_vec(&self.key) {
            Ok(mut bytes) => {
                // Add newline delimiter for TCP stream parsing
                bytes.push(b'\n');

                if let Err(err) = state.connection_handler.tcp_tx.send(bytes.clone()) {
                    eprintln!("Failed to send to internal stream: {}", err);
                }

                // Tells the remote server to enable RCON
                //if let Some(internal_tx) = &state.internal_tx {
                if let Err(err) = state.connection_handler.tcp_tx.send(bytes) {
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
    type Output = ();
    async fn node_transport(
        &self,
        state: &mut AppState,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        let server_state_request = proto::ServerStateRequest {};
        let _ = state
            .connection_handler
            .clients
            .as_mut()
            .unwrap()
            .server_manage_client
            .state(server_state_request);
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
impl Into<proto::MetadataTypes> for MetadataTypes {
    fn into(self) -> proto::MetadataTypes {
        serde_json::from_value(serde_json::to_value(self.clone()).unwrap()).unwrap()
    }
}

