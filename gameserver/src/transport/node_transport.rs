use std::{any::Any, error::Error};

use crate::MessagePayload;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use serde_json::Value;
use tcp_filesystem::FileRequestMessage;
use tokio::{io::{AsyncReadExt, AsyncWriteExt}, net::{tcp::{OwnedReadHalf, OwnedWriteHalf}, TcpListener, TcpStream}};

use crate::{AppState, GetState, IncomingMessage, IncomingMessageWithMetadata, SimpleMessage};

pub struct ConnectionManager {
    listner: TcpListener,
    connections: Vec<ConnectionManager>
}
impl ConnectionManager {
    pub async fn serve(url: String) -> Result<ConnectionManager, Box<dyn std::error::Error + Send + Sync>> {
        let listner = TcpListener::bind(url).await?;
    
        Ok(
            ConnectionManager { 
                listner, 
                connections: vec![]
            }
        )
    }
    pub async fn accept_connection(
        &mut self,
    ) -> Result<(ConnectionHandler, Option<String>), Box<dyn std::error::Error + Send + Sync>> {
        let (socket, addr) = self.listner.accept().await?;
        let handler = ConnectionHandler {
            stream: Some(socket),
            read_buf: vec![],
            newline_pos: 0,
            //last_request: None
        };
        Ok((handler, Some(addr.to_string())))
    }
}

pub struct ConnectionHandler {
    stream: Option<TcpStream>,
    read_buf: Vec<u8>,
    newline_pos: usize,
    //last_request: Option<String>
}

impl ConnectionHandler {
    pub fn new() -> ConnectionHandler {
        ConnectionHandler {
            stream: None,
            read_buf: Vec::new(),
            newline_pos: 0,
            //last_request: None
        }
    }

    pub fn inner(&mut self) -> &mut Vec<u8> {
        &mut self.read_buf
    }

    pub fn clear(&self) {}
    pub fn remove_current_segment_or_clear(&mut self) {
        self.remove_segment_or_clear(self.newline_pos);
    }
    fn remove_segment_or_clear(&mut self, position: usize) {
        if position + 1 <= self.inner().len() {
            self.inner().drain(..position + 1);
        } else {
            self.inner().clear();
        }
    }
    //pub fn next() -> Option<usize> {
    pub async fn next(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if let Some(pos) = self.read_buf.iter().position(|&b| b == b'\n') {
            self.newline_pos = pos;
            //self.remove_current_segment_or_clear();
            Ok(())
        } else {
            Err("Did not find next position".into())
        }
        //todo!()
    }
    pub async fn recv_line(&mut self) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        let newline_pos = self.newline_pos.clone();
        let line = &self.read_buf[..newline_pos];

        if line.is_empty() {
            self.remove_current_segment_or_clear();
            return Err("Line is empty".into());
        }

        let line_str = String::from_utf8_lossy(line);
        // if let Some(last_request) = &self.last_request {
        //     if *last_request == line_str {
        //         return Err("Line is a duplicate".into());
        //     }
        // }
        // self.last_request = Some(line_str.to_string());

        //self.remove_current_segment_or_clear();
        Ok(line_str.to_string())
    }
    pub async fn append_bytes(&mut self, bytes: Vec<u8>) {
        self.inner().extend_from_slice(&bytes);
    }
    pub async fn has_remaining_buffer(&self) -> bool {
        self.newline_pos + 1 <= self.read_buf.len()
    }
    pub async fn handle_request(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // read_half.read(&mut temp_buf) => {
        //     let n = match result {
        //         Ok(0) => break,
        //         Ok(n) => n,
        //         Err(e) => {
        //             eprintln!("[{}] Read error: {}", addr, e);
        //             break;
        //         }
        //     };
        //     read_buf.append_bytes((&temp_buf[..n]).to_vec()).await;
        //     // read_buf.inner().extend_from_slice(&temp_buf[..n]);
        // }
        if let Some(stream) = &mut self.stream {
            let mut temp_buf = vec![0u8; 4096];
            stream.read(&mut temp_buf).await?;
            Ok(())
        } else {
            Err("no stream exists".into())
        }
        //Ok(())
    }
    pub async fn send(
        &mut self,
        bytes: Vec<u8>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if let Some(stream) = &mut self.stream {
            stream.write_all(&bytes).await?;
            Ok(())
        } else {
            Err("no stream".into())
        }
    }
    pub async fn recv(&self) -> Result<Vec<u8>, Box<dyn std::error::Error + Send + Sync>> {
        todo!()
    }
    // pub async fn accept_connection(
    //     &self,
    // ) -> Result<Option<String>, Box<dyn std::error::Error + Send + Sync>> {
    //     Ok(None)
    // }
    pub fn split(&mut self) -> Result<(Writer, Reader), Box<dyn std::error::Error + Send + Sync>> {
        let stream = self.stream.take().ok_or("no stream set")?;
        let (read_half, write_half) = stream.into_split();
        // Ok((Writer { write_half }, Reader { read_half, read_buf: Some(&self.read_buf) }))
        Ok((Writer { write_half }, Reader { read_half }))
    }
    // pub async fn handle_connections() {
    // }
}
pub struct Writer {
    write_half: OwnedWriteHalf,
}
impl Writer {
    pub async fn send(
        &mut self,
        bytes: Vec<u8>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.write_half.write_all(&bytes).await?;
        Ok(())
    }
    pub async fn send_with_connection(
        &mut self,
        bytes: Vec<u8>,
        handler: &mut ConnectionHandler
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.write_half.write_all(&bytes).await?;
        Ok(())
    }
}
pub struct Reader {
    read_half: OwnedReadHalf,
    //read_buf: Option<&Vec<u8>>,
}
impl Reader {
    pub async fn recv(&mut self) -> Result<Vec<u8>, Box<dyn std::error::Error + Send + Sync>> {
        let mut temp_buf = vec![0u8; 4096];
        let n = self.read_half.read(&mut temp_buf).await?;

        if n == 0 {
            return Err("connection closed by peer or no bytes".into());
        }

        println!("got {}", String::from_utf8_lossy(&temp_buf[..n]));
        Ok(temp_buf)
    }
    pub async fn handle_request(&mut self, handler: &mut ConnectionHandler) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // read_half.read(&mut temp_buf) => {
        //     let n = match result {
        //         Ok(0) => break,
        //         Ok(n) => n,
        //         Err(e) => {
        //             eprintln!("[{}] Read error: {}", addr, e);
        //             break;
        //         }
        //     };
        //     read_buf.append_bytes((&temp_buf[..n]).to_vec()).await;
        //     // read_buf.inner().extend_from_slice(&temp_buf[..n]);
        // }
        // let mut temp_buf = vec![0u8; 4096];
        // self.read_half.read(&mut temp_buf).await?;
        // println!("got {}", String::from_utf8_lossy(&temp_buf));
        // Ok(())
        let mut temp_buf = vec![0u8; 4096];
        let n = self.read_half.read(&mut temp_buf).await?;

        if n == 0 {
            return Err("connection closed by peer or no bytes".into());
        }
        handler.append_bytes(temp_buf.clone()).await;
        Ok(())
    }
}

#[derive(Serialize, Deserialize, Clone)]
pub enum RequestTypes {
    Ping,
    CreateServerRequest(CreateServerRequest),
    DeleteSeverRequest(DeleteServerRequest),
    SetFilterRequest(SetFilterRequest),
    SetServerRequest(SetServerRequest),
    ServerDataRequest(ServerDataRequest),
    ServerNameRequest(ServerNameRequest),
    StartServerRequest(StartServerRequest),
    StopServerRequest(StopServerRequest),
    ServerStateRequest(ServerStateRequest),
    ConsoleRequest(ConsoleRequest),
    FileRequest(FileRequest),
}

pub struct RequestHandler {}
impl RequestHandler {
    pub fn try_recv_req(value: Value) -> Option<RequestTypes> {
        //if let Ok(ping_request)
        if let Ok(create_server_request) =
            serde_json::from_value::<CreateServerRequest>(value.clone())
        {
            if create_server_request.common.message == "create_server" {
                return Some(RequestTypes::CreateServerRequest(create_server_request));
            }
        }
        if let Ok(delete_server_request) =
            serde_json::from_value::<DeleteServerRequest>(value.clone())
        {
            if delete_server_request.common.message == "delete_server" {
                return Some(RequestTypes::DeleteSeverRequest(delete_server_request));
            }
        }
        if let Ok(set_server_request) = serde_json::from_value::<SetServerRequest>(value.clone()) {
            if set_server_request.common.message == "set_server" {
                return Some(RequestTypes::SetServerRequest(set_server_request));
            }
        }
        if let Ok(start_server_request) =
            serde_json::from_value::<StartServerRequest>(value.clone())
        {
            if start_server_request.common.message == "start_server" {
                return Some(RequestTypes::StartServerRequest(start_server_request));
            }
        }
        if let Ok(stop_server_request) = serde_json::from_value::<StopServerRequest>(value.clone())
        {
            if stop_server_request.common.message == "stop_server" {
                return Some(RequestTypes::StopServerRequest(stop_server_request));
            }
        }

        if let Ok(server_data_request) = serde_json::from_value::<ServerDataRequest>(value.clone())
        {
            if server_data_request.common.message == "server_data" {
                return Some(RequestTypes::ServerDataRequest(server_data_request));
            }
        }

        if let Ok(server_name_request) = serde_json::from_value::<ServerNameRequest>(value.clone())
        {
            if server_name_request.common.message == "server_name" {
                return Some(RequestTypes::ServerNameRequest(server_name_request));
            }
        }

        if let Ok(server_state_request) =
            serde_json::from_value::<ServerStateRequest>(value.clone())
        {
            if server_state_request.common.message == "server_state" {
                return Some(RequestTypes::ServerStateRequest(server_state_request));
            }
        }
        if let Ok(console_request) = serde_json::from_value::<ConsoleRequest>(value.clone()) {
            if console_request.common.message_type == "console" {
                return Some(RequestTypes::ConsoleRequest(console_request));
            }
        }
        if let Ok(ping_request) = serde_json::from_value::<SimpleMessage>(value.clone()) {
            if ping_request.message == "ping" {
                return Some(RequestTypes::Ping);
            }
        }
        if let Ok(set_filter_request) = serde_json::from_value::<SetFilterRequest>(value.clone()) {
            if set_filter_request.common.message == "set_filter" {
                return Some(RequestTypes::SetFilterRequest(set_filter_request));
            }
        }
        //println!("{:#?}", serde_json::from_value::<FileRequest>(value.clone()).iter().cloned());
        if let Ok(file_request) = serde_json::from_value::<FileRequest>(value.clone()) {
            return Some(RequestTypes::FileRequest(file_request));
        }

        None
    }
}

impl TryIntoRequest for RequestTypes {
    type Request = Self;

    fn into_typed_request<T: 'static>(self) -> Result<T, Box<dyn std::error::Error + Send + Sync>> {
        let boxed: Box<dyn Any> = match self {
            RequestTypes::CreateServerRequest(req) => Box::new(req),
            RequestTypes::Ping => Box::new(Ping::default()),
            RequestTypes::DeleteSeverRequest(delete_server_request) => {
                Box::new(delete_server_request)
            }
            RequestTypes::SetFilterRequest(set_filter_request) => Box::new(set_filter_request),
            RequestTypes::SetServerRequest(set_server_request) => Box::new(set_server_request),
            RequestTypes::ServerDataRequest(server_data_request) => Box::new(server_data_request),
            RequestTypes::ServerNameRequest(server_name_request) => Box::new(server_name_request),
            RequestTypes::StartServerRequest(start_server_request) => {
                Box::new(start_server_request)
            }
            RequestTypes::StopServerRequest(stop_server_request) => Box::new(stop_server_request),
            RequestTypes::ServerStateRequest(server_state_request) => {
                Box::new(server_state_request)
            }
            RequestTypes::ConsoleRequest(console_request) => Box::new(console_request),
            RequestTypes::FileRequest(file_request) => Box::new(file_request),
        };

        boxed
            .downcast::<T>()
            .map(|b| *b)
            .map_err(|_| "Type T did not match the inner request type".into())
    }

    fn into_request(
        value: Value,
    ) -> Result<Self::Request, Box<dyn std::error::Error + Send + Sync>> {
        Err("Not implimented".into())
    }
}

//FileRequestMessage
#[derive(Deserialize, Serialize, Clone, Debug)]
pub struct FileRequest {
    #[serde(flatten)]
    pub common: FileRequestMessage,
}

#[derive(Deserialize, Serialize, Clone)]
pub struct ConsoleRequest {
    #[serde(flatten)]
    pub common: IncomingMessage,
}

#[derive(Deserialize, Serialize, Clone)]
pub struct ServerStateRequest {
    #[serde(flatten)]
    pub common: IncomingMessage,
}

#[derive(Deserialize, Serialize, Clone)]
pub struct StopServerRequest {
    #[serde(flatten)]
    pub common: IncomingMessage,
}

#[derive(Deserialize, Serialize, Clone)]
pub struct StartServerRequest {
    #[serde(flatten)]
    pub common: IncomingMessage,
}

#[derive(Deserialize, Serialize, Clone)]
pub struct ServerNameRequest {
    #[serde(flatten)]
    pub common: IncomingMessage,
}

#[derive(Deserialize, Serialize, Clone)]
pub struct ServerDataRequest {
    #[serde(flatten)]
    pub common: IncomingMessage,
}

#[derive(Deserialize, Serialize, Clone)]
pub struct CreateServerRequest {
    #[serde(flatten)]
    pub common: IncomingMessageWithMetadata,
}

#[derive(Deserialize, Serialize, Clone)]
pub struct DeleteServerRequest {
    #[serde(flatten)]
    pub common: IncomingMessageWithMetadata,
}

#[derive(Deserialize, Serialize, Clone)]
pub struct SetServerRequest {
    #[serde(flatten)]
    pub common: IncomingMessageWithMetadata,
}

#[derive(Deserialize, Serialize, Clone)]
pub struct SetFilterRequest {
    #[serde(flatten)]
    pub common: IncomingMessageWithMetadata,
}

#[derive(Deserialize, Serialize, Default, Clone)]
pub struct Ping {
    #[serde(flatten)]
    pub common: SimpleMessage,
}

pub trait TryIntoRequest {
    type Request: DeserializeOwned + Serialize;

    fn into_typed_request<T: 'static>(self) -> Result<T, Box<dyn std::error::Error + Send + Sync>>;

    fn into_request(
        value: Value,
    ) -> Result<Self::Request, Box<dyn std::error::Error + Send + Sync>>;
}

pub trait NodeTransportable {
    async fn node_transport(&self, state: &AppState) -> Result<(), Box<dyn Error + Send + Sync>>;
}

pub struct ServerDataResponse {
    pub state: GetState,
}
impl NodeTransportable for ServerDataResponse {
    async fn node_transport(&self, state: &AppState) -> Result<(), Box<dyn Error + Send + Sync>> {
        let tx = state.output_tx.lock().await.clone().unwrap();
        let _ = tx.send(serde_json::to_string(&self.state)?).await;
        Ok(())
    }
}

pub struct PingResponse {
    pub message: SimpleMessage,
}
impl NodeTransportable for PingResponse {
    async fn node_transport(&self, state: &AppState) -> Result<(), Box<dyn Error + Send + Sync>> {
        let tx = state.output_tx.lock().await.clone().unwrap();
        let _ = tx.send(serde_json::to_string(&self.message)?).await;
        Ok(())
    }
}

#[derive(Debug)]
pub struct FileOperationResponse {
    pub data: String,
}
impl NodeTransportable for FileOperationResponse {
    async fn node_transport(&self, state: &AppState) -> Result<(), Box<dyn Error + Send + Sync>> {
        let tx = state.output_tx.lock().await.clone().unwrap();
        let _ = tx.send(self.data.clone()).await;
        Ok(())
    }
}

pub struct ServerNameResponse {
    pub message: MessagePayload,
}
impl NodeTransportable for ServerNameResponse {
    async fn node_transport(&self, state: &AppState) -> Result<(), Box<dyn Error + Send + Sync>> {
        let tx = state.output_tx.lock().await.clone().unwrap();
        let _ = tx.send(serde_json::to_string(&self.message)?).await;
        Ok(())
    }
}

pub struct ServerStateResponse {
    pub message: MessagePayload,
}
impl NodeTransportable for ServerStateResponse {
    async fn node_transport(&self, state: &AppState) -> Result<(), Box<dyn Error + Send + Sync>> {
        let tx = state.output_tx.lock().await.clone().unwrap();
        let _ = tx.send(serde_json::to_string(&self.message)?).await;
        Ok(())
    }
}
