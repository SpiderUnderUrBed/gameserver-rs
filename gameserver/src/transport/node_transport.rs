use std::sync::Arc;

use crate::{AppState, MessagePayload};
use network_abstraction_lib::{FromWire, Router, ValueRequest};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use serde_json::Value;

use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{
        tcp::{OwnedReadHalf, OwnedWriteHalf},
        TcpListener, TcpStream,
    }, sync::Mutex,
};

use crate::{GetState, IncomingMessage, IncomingMessageWithMetadata, SimpleMessage};

pub enum TaggedRequest {
    None,
    Fs(Vec<u8>),
}

pub struct ConnectionManager {
    listner: TcpListener,
    connections: Vec<ConnectionManager>,
    router: Arc<Mutex<Router<Arc<AppState>>>>
}
impl ConnectionManager {
    pub async fn serve(
        router: Router<Arc<AppState>>,
        url: String,
    ) -> Result<ConnectionManager, Box<dyn std::error::Error + Send + Sync>> {
        let listner = TcpListener::bind(url).await?;

        Ok(ConnectionManager {
            listner,
            connections: vec![],
            router: Arc::new(Mutex::new(router)),
        })
    }
    pub async fn accept_connection(
        &mut self,
    ) -> Result<(ConnectionHandler, Option<String>), Box<dyn std::error::Error + Send + Sync>> {
        let (socket, addr) = self.listner.accept().await?;
        let handler = ConnectionHandler {
            stream: Some(socket),
            read_buf: vec![],
            newline_pos: 0,
        };
        Ok((handler, Some(addr.to_string())))
    }
    pub async fn get_arc_mutex_router(&self) -> Arc<Mutex<Router<Arc<AppState>>>>{
       self.router.clone()
    }
}

pub struct ConnectionHandler {
    stream: Option<TcpStream>,
    read_buf: Vec<u8>,
    newline_pos: usize,
}

impl ConnectionHandler {
    pub fn new() -> ConnectionHandler {
        ConnectionHandler {
            stream: None,
            read_buf: Vec::new(),
            newline_pos: 0,
        }
    }

    pub fn inner(&mut self) -> &mut Vec<u8> {
        &mut self.read_buf
    }

    pub fn clear(&self) {}
    pub async fn start_clean_hook(&mut self) {
        self.remove_current_segment_or_clear().await;
    }
    pub async fn end_clean_hook(&mut self) {
        self.remove_current_segment_or_clear().await;
    }
    pub async fn remove_current_segment_or_clear(&mut self) {
        self.remove_segment_or_clear(self.newline_pos);
    }
    fn remove_segment_or_clear(&mut self, position: usize) {
        if position + 1 <= self.inner().len() {
            self.inner().drain(..position + 1);
        } else {
            self.inner().clear();
        }
    }
    pub async fn next(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if self.read_buf.len() > 0 {
        }
        if let Some(pos) = self.read_buf.iter().position(|&b| b == b'\n') {
            self.newline_pos = pos;
            Ok(())
        } else {
            Err("Did not find next position".into())
        }
    }
    pub fn recv_bytes(&mut self) -> Vec<u8> {
        let bytes = self.read_buf.clone();
        self.read_buf = Vec::new();
        bytes
    }
    pub async fn recv_line(&mut self) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        let newline_pos = self.newline_pos.clone();
        let line = &self.read_buf[..newline_pos];

        if line.is_empty() {
            self.remove_current_segment_or_clear().await;
            return Err("Line is empty".into());
        }

        let line_str = String::from_utf8_lossy(line);
        Ok(line_str.to_string())
    }
    pub async fn append_bytes(&mut self, bytes: Vec<u8>) {
        self.inner().extend_from_slice(&bytes);
    }
    pub async fn has_remaining_buffer(&self) -> bool {
        self.newline_pos + 1 <= self.read_buf.len()
    }
    pub async fn handle_request(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if let Some(stream) = &mut self.stream {
            let mut temp_buf = vec![0u8; 4096];
            stream.read(&mut temp_buf).await?;
            Ok(())
        } else {
            Err("no stream exists".into())
        }
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
    pub fn split(&mut self) -> Result<(Writer, Reader), Box<dyn std::error::Error + Send + Sync>> {
        let stream = self.stream.take().ok_or("no stream set")?;
        let (read_half, write_half) = stream.into_split();
        Ok((Writer { write_half }, Reader { read_half }))
    }
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
        _handler: &mut ConnectionHandler,
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
    pub async fn handle_request(
        &mut self,
        handler: &mut ConnectionHandler,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let mut temp_buf = vec![0u8; 4096];
        let n = {
            match self.read_half.read(&mut temp_buf).await {
                Ok(n) => {
                    n
                },
                Err(e) => {
                    return Err("failed to read".into());
                }
            }
        };
        if n == 0 {
            return Err("connection closed by peer or no bytes".into());
        }
        handler.append_bytes(temp_buf.clone()).await;
        Ok(())
    }
}

#[derive(Deserialize, Serialize, Clone)]
pub struct ConsoleRequest {
    #[serde(flatten)]
    pub common: IncomingMessage,
}
impl FromWire for ConsoleRequest {
    type Request = ValueRequest;

    type Error = serde_json::Error;

    fn from_wire(req: Self::Request) -> Result<Self, Self::Error> {
        serde_json::from_value(req.value)
    }
}

#[derive(Deserialize, Serialize, Clone)]
pub struct ServerStateRequest {
    #[serde(flatten)]
    pub common: IncomingMessage,
}
impl FromWire for ServerStateRequest {
    type Request = ValueRequest;

    type Error = serde_json::Error;

    fn from_wire(req: Self::Request) -> Result<Self, Self::Error> {
        serde_json::from_value(req.value)
    }
}

#[derive(Deserialize, Serialize, Clone)]
pub struct StopServerRequest {
    #[serde(flatten)]
    pub common: IncomingMessage,
}
impl FromWire for StopServerRequest {
    type Request = ValueRequest;

    type Error = serde_json::Error;

    fn from_wire(req: Self::Request) -> Result<Self, Self::Error> {
        serde_json::from_value(req.value)
    }
}

#[derive(Deserialize, Serialize, Clone)]
pub struct StartServerRequest {
    #[serde(flatten)]
    pub common: IncomingMessage,
}
impl FromWire for StartServerRequest {
    type Request = ValueRequest;

    type Error = serde_json::Error;

    fn from_wire(req: Self::Request) -> Result<Self, Self::Error> {
        serde_json::from_value(req.value)
    }
}

#[derive(Deserialize, Serialize, Clone)]
pub struct ServerNameRequest {
    #[serde(flatten)]
    pub common: IncomingMessage,
}
impl FromWire for ServerNameRequest {
    type Request = ValueRequest;

    type Error = serde_json::Error;

    fn from_wire(req: Self::Request) -> Result<Self, Self::Error> {
        serde_json::from_value(req.value)
    }
}

#[derive(Deserialize, Serialize, Clone)]
pub struct ServerDataRequest {
    #[serde(flatten)]
    pub common: IncomingMessage,
}
impl FromWire for ServerDataRequest {
    type Request = ValueRequest;

    type Error = serde_json::Error;

    fn from_wire(req: Self::Request) -> Result<Self, Self::Error> {
        serde_json::from_value(req.value)
    }
}

#[derive(Deserialize, Serialize, Clone)]
pub struct CreateServerRequest {
    #[serde(flatten)]
    pub common: IncomingMessageWithMetadata,
}
impl FromWire for CreateServerRequest {
    type Request = ValueRequest;

    type Error = serde_json::Error;

    fn from_wire(req: Self::Request) -> Result<Self, Self::Error> {
        serde_json::from_value(req.value)
    }
}

#[derive(Deserialize, Serialize, Clone)]
pub struct DeleteServerRequest {
    #[serde(flatten)]
    pub common: IncomingMessageWithMetadata,
}

impl FromWire for DeleteServerRequest {
    type Request = ValueRequest;

    type Error = serde_json::Error;

    fn from_wire(req: Self::Request) -> Result<Self, Self::Error> {
        serde_json::from_value(req.value)
    }
}

#[derive(Deserialize, Serialize, Clone)]
pub struct SetServerRequest {
    #[serde(flatten)]
    pub common: IncomingMessageWithMetadata,
}

impl FromWire for SetServerRequest {
    type Request = ValueRequest;

    type Error = serde_json::Error;

    fn from_wire(req: Self::Request) -> Result<Self, Self::Error> {
        serde_json::from_value(req.value)
    }
}

#[derive(Deserialize, Serialize, Clone)]
pub struct SetFilterRequest {
    #[serde(flatten)]
    pub common: IncomingMessageWithMetadata,
}

impl FromWire for SetFilterRequest {
    type Request = ValueRequest;

    type Error = serde_json::Error;

    fn from_wire(req: Self::Request) -> Result<Self, Self::Error> {
        serde_json::from_value(req.value)
    }
}

#[derive(Deserialize, Serialize, Default, Clone)]
pub struct Ping {
    #[serde(flatten)]
    pub common: SimpleMessage,
}

impl FromWire for Ping {
    type Request = ValueRequest;

    type Error = serde_json::Error;

    fn from_wire(req: Self::Request) -> Result<Self, Self::Error> {
        serde_json::from_value(req.value)
    }
}


#[derive(Serialize)]
pub struct ServerDataResponse {
    pub state: GetState,
}

#[derive(Serialize)]
pub struct PingResponse {
    pub message: SimpleMessage,
}

#[derive(Serialize)]
pub struct ServerNameResponse {
    pub message: MessagePayload,
}

#[derive(Serialize)]
pub struct ServerStateResponse {
    pub message: MessagePayload,
}