

use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::{net::TcpStream, sync::{broadcast, RwLock}, time::{sleep, timeout}};
use tokio_util::sync::CancellationToken;

use crate::{
    database::databasespec::Filters, handle_stream, AppState, MessagePayload, MessagePayloadWithMetadata, MetadataTypes, SimpleMessage, SrcAndDest, Status, StreamResult, CONNECTION_RETRY_DELAY, CONNECTION_TIMEOUT
};
use crate::CHANNEL_BUFFER_SIZE;
use crate::{AuthTcpMessage, ApiCalls as ToplevelApiCalls, List, IncomingMessage, NodeAndTCP};
use std::{error::Error, net::SocketAddr, sync::Arc, time::Instant};
use std::time::Duration;
use anyhow::anyhow;

mod proto {
    tonic::include_proto!("main");
}
use proto::{general_client::GeneralClient, DeleteServerRequest as DeleteServerRequestGrpc, server_edit_server::ServerEdit};





// does the connection to the tcp server, wether initial or not, on success it will pass it off to the dedicated handler for the stream
pub async fn connect_to_server(
    arc_state: Arc<RwLock<AppState>>,
    mut tcp_url: String,
    ws_tx: broadcast::Sender<String>,
    end_if_timeout: bool,
    block_with_stream: bool,
) -> Result<Option<SocketAddr>, Box<dyn Error + Send + Sync>> {
    // let server_edit = 
    let mut client = GeneralClient::connect(tcp_url).await;

    Ok(None)
}

// for the initial connection attempt, which will determine if possibly I would need to create the container and deployment upon failure
// i will use rusts 'timeout' for x interval determined with CONNECTION_TIMEOUT
pub async fn attempt_connection(
    tcp_url: String,
) -> Result<TcpStream, Box<dyn std::error::Error + Send + Sync>> {
    timeout(CONNECTION_TIMEOUT, TcpStream::connect(tcp_url))
        .await?
        .map_err(Into::into)
}


// this is where it determines wether or not to try and create the container and deployment, as attempt_connection itself is used in various diffrent contexts (like it will constantly
// try to connect upon failing but it should not try to create the container and deployment every time it fails)
// I use anyhow here because it saves me having to try and downcast the error type
pub async fn try_initial_connection(
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
                    let ip = stream.peer_addr()?.ip().to_string();

                    let stream_result = handle_stream(
                        state.clone(),
                        &mut temp_rx,
                        //&mut stream,
                        ip,
                        ws_tx.clone(),
                        None,
                    )
                    .await;
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

// #[tonic::async_trait]
// impl ServerEdit for DeleteServerRequestGrpc {
//     async fn create(
//         &self,
//         request: tonic::Request<proto::CreateServerRequest>
//     ) -> Result<tonic::Response<proto::CreateServerResponse>, tonic::Status>{
//         let input = request.get_ref();
//     }
// }

// pub enum RequestTypes {
//     Ping,
//     CreateServerRequest(CreateServerRequest),
//     DeleteSeverRequest(DeleteServerRequest),
//     SetFilterRequest(SetFilterRequest),
//     SetServerRequest(SetServerRequest),
//     ServerDataRequest(ServerDataRequest),
//     ServerNameRequest(ServerNameRequest),
//     StartServerRequest(StartServerRequest),
//     StopServerRequest(StopServerRequest),
//     ServerStateRequest(ServerStateRequest),
//     ConsoleRequest(ConsoleRequest),
//     FileRequest(FileRequest),
// }

// pub struct RequestHandler {}
// impl RequestHandler {
//     pub fn try_recv_req(value: Value) -> Option<RequestTypes> {

// pub trait TryIntoRequest {
//     type Request: DeserializeOwned + Serialize;

//     fn into_typed_request<T: 'static>(self) -> Result<T, Box<dyn std::error::Error + Send + Sync>>;

//     fn into_request(
//         value: Value,
//     ) -> Result<Self::Request, Box<dyn std::error::Error + Send + Sync>>;
// }

// impl TryIntoRequest for RequestTypes {
//     type Request = Self;

//     fn into_typed_request<T: 'static>(self) -> Result<T, Box<dyn std::error::Error + Send + Sync>> {
//         let boxed: Box<dyn Any> = match self {
//             RequestTypes::CreateServerRequest(req) => Box::new(req),
//             RequestTypes::Ping => Box::new(Ping::default()),
//             RequestTypes::DeleteSeverRequest(delete_server_request) => {
//                 Box::new(delete_server_request)
//             }
//             RequestTypes::SetFilterRequest(set_filter_request) => Box::new(set_filter_request),
//             RequestTypes::SetServerRequest(set_server_request) => Box::new(set_server_request),
//             RequestTypes::ServerDataRequest(server_data_request) => Box::new(server_data_request),
//             RequestTypes::ServerNameRequest(server_name_request) => Box::new(server_name_request),
//             RequestTypes::StartServerRequest(start_server_request) => {
//                 Box::new(start_server_request)
//             }
//             RequestTypes::StopServerRequest(stop_server_request) => Box::new(stop_server_request),
//             RequestTypes::ServerStateRequest(server_state_request) => {
//                 Box::new(server_state_request)
//             }
//             RequestTypes::ConsoleRequest(console_request) => Box::new(console_request),
//             RequestTypes::FileRequest(file_request) => Box::new(file_request),
//         };

//         boxed
//             .downcast::<T>()
//             .map(|b| *b)
//             .map_err(|_| "Type T did not match the inner request type".into())
//     }

//     fn into_request(
//         value: Value,
//     ) -> Result<Self::Request, Box<dyn std::error::Error + Send + Sync>> {
//         Err("Not implimented".into())
//     }
// }



pub trait NodeTransportable {
    async fn node_transport(&self, state: &AppState) -> Result<(), Box<dyn Error + Send + Sync>>;
}

pub trait ImmediateTransportable {
    async fn immediate_transport(&self, state: &mut AppState) -> Result<(), Box<dyn Error + Send + Sync>>;
}


pub struct PasswordRequest {
    pub password: String
}
pub struct CapabilitiesRequest {
    pub capabilities: Vec<String>
}
pub struct ServernameRequest {
    pub ip: String
}
impl ImmediateTransportable for PasswordRequest {
    async fn immediate_transport(&self, state: &mut AppState) -> Result<(), Box<dyn Error + Send + Sync>> {
        let auth_msg = serde_json::to_vec(&AuthTcpMessage {
            password: self.password.clone(),
        })?;
        state.connection_handler.tcp_tx.send(auth_msg);
        Ok(())
    }
}
impl ImmediateTransportable for CapabilitiesRequest {
    async fn immediate_transport(&self, state: &mut AppState) -> Result<(), Box<dyn Error + Send + Sync>> {
        let capability_msg = serde_json::to_vec(&List {
            list: ToplevelApiCalls::Capabilities(self.capabilities.clone()),
        })?;
        let _ = state.connection_handler.tcp_tx.send(capability_msg);
        Ok(())
    }
}
impl ImmediateTransportable for ServernameRequest {
    async fn immediate_transport(&self, state: &mut AppState) -> Result<(), Box<dyn Error + Send + Sync>> {
        let cmd_msg = serde_json::to_vec(&MessagePayload {
            r#type: "command".to_string(),
            message: "server_name".to_string(),
            authcode: "0".to_string(),
        })?;
        let _ = state.connection_handler.tcp_tx.send(cmd_msg);
        // writer.write_all(cmd_msg.as_bytes()).await?;

        'name: {
            // let mut state = arc_state.write().await;
            if let Ok(Ok(bytes)) = timeout(Duration::from_millis(1000), state.connection_handler.tcp_rx.recv()).await
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

// pub fn handle_password(state: &AppState, password: String) -> Result<(), Box<dyn Error + Send + Sync>>{
//     // let initial_node_password: String =
//     //     get_env_var_or_arg("INITIAL_NODE_PASSWORD", Some(String::default())).unwrap();
//     let auth_msg = serde_json::to_vec(&AuthTcpMessage {
//         password,
//     })?;
//     let _ = state.tcp_tx.send(auth_msg);
//     // writer.write_all(auth_msg.as_bytes()).await?;
//     Ok(())
// }
// pub fn handle_capabilities(state: &AppState, capabilities: Vec<String>) -> Result<(), Box<dyn Error + Send + Sync>>{
//     let capability_msg = serde_json::to_vec(&List {
//         list: ToplevelApiCalls::Capabilities(capabilities),
//     })?;
//     let _ = state.tcp_tx.send(capability_msg);
//     // writer.write_all(capability_msg.as_bytes()).await?;


//     Ok(())
// }
// pub async fn handle_servername(state: &mut AppState, ip: String) -> Result<(), Box<dyn Error + Send + Sync>>{
//     let cmd_msg = serde_json::to_vec(&MessagePayload {
//         r#type: "command".to_string(),
//         message: "server_name".to_string(),
//         authcode: "0".to_string(),
//     })?;
//     let _ = state.tcp_tx.send(cmd_msg);
//     // writer.write_all(cmd_msg.as_bytes()).await?;

//     'name: {
//         // let mut state = arc_state.write().await;
//         if let Ok(Ok(bytes)) = timeout(Duration::from_millis(1000), state.tcp_rx.recv()).await
//         {
//             if let Ok(payload) = serde_json::from_slice::<IncomingMessage>(&bytes) {
//                 state.current_node = NodeAndTCP {
//                     name: payload.message,
//                     ip: ip.clone(),
//                     ..Default::default()
//                 };
//                 break 'name;
//             }
//         }
//         state.current_node = NodeAndTCP {
//             name: "main".to_string(),
//             ip: ip.clone(),
//             ..Default::default()
//         };
//     }

//     Ok(())
// }
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

        let _ = state.connection_handler.tcp_tx.send(bytes);

        Ok(())
    }
}

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
        let _ = state.connection_handler.tcp_tx.send(bytes);

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
        let _ = state.connection_handler.tcp_tx.send(msg.unwrap());

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
        let _ = state.connection_handler.tcp_tx.send(msg.unwrap());

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
        let _ = state.connection_handler.tcp_tx.send(bytes);

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
        let _ = state.connection_handler.tcp_tx.send(bytes);

        Ok(())
    }
}

// struct ServerState
// NodeTransportable

pub struct RawBytes {
    pub(crate) bytes: Vec<u8>,
}

impl NodeTransportable for RawBytes {
    async fn node_transport(&self, state: &AppState) -> Result<(), Box<dyn Error + Send + Sync>> {
        let _ = state.connection_handler.tcp_tx.send(self.bytes.clone());
        Ok(())
    }
}
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

// }
pub struct Ping {}
impl NodeTransportable for Ping {
    async fn node_transport(&self, state: &AppState) -> Result<(), Box<dyn Error + Send + Sync>> {
        let ping = SimpleMessage {
            message: "ping".to_string(),
        };
        let res = state.connection_handler.tcp_tx.send(serde_json::to_vec(&ping).unwrap());

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
    async fn node_transport(&self, state: &AppState) -> Result<(), Box<dyn Error + Send + Sync>> {
        let msg = serde_json::to_vec(&MessagePayload {
            r#type: "command".to_string(),
            message: "server_state".to_string(),
            authcode: "0".to_string(),
        })
        .unwrap();
        let _ = state.connection_handler.tcp_tx.send(msg);

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
