use std::{any::Any, error::Error};

use crate::MessagePayload;
use network_abstraction_lib::{FromWire, IntoResponse, ValueRequest};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use serde_json::Value;

use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{
        tcp::{OwnedReadHalf, OwnedWriteHalf},
        TcpListener, TcpStream,
    },
};

use crate::{AppState, GetState, IncomingMessage, IncomingMessageWithMetadata, SimpleMessage};

// #[derive(Serialize, Deserialize, Debug, Clone)]
// pub struct FileResponseMessage {
//     pub in_response_to: u64,
//     pub data: Vec<u8>,
// }

// #[derive(Serialize, Deserialize, Clone, Debug)]
// pub struct FileRequestMessage {
//     pub id: u64,
//     #[serde(flatten)]
//     pub payload: FileRequestPayload,
// }
pub enum TaggedRequest {
    None,
    Fs(Vec<u8>),
}

pub struct ConnectionManager {
    listner: TcpListener,
    connections: Vec<ConnectionManager>,
}
impl ConnectionManager {
    pub async fn serve(
        url: String,
    ) -> Result<ConnectionManager, Box<dyn std::error::Error + Send + Sync>> {
        let listner = TcpListener::bind(url).await?;

        Ok(ConnectionManager {
            listner,
            connections: vec![],
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
    //pub fn next() -> Option<usize> {
    pub async fn next(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if let Some(pos) = self.read_buf.iter().position(|&b| b == b'\n') {
            //println!("{:#?}", self.read_buf.iter().filter(|i| **i != 0).collect::<Vec<&u8>>());
            self.newline_pos = pos;
            //self.remove_current_segment_or_clear();
            Ok(())
        } else {
            Err("Did not find next position".into())
        }
        //todo!()
    }
    pub fn recv_tagged(&mut self) -> TaggedRequest {
        TaggedRequest::None
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
        handler: &mut ConnectionHandler,
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

// //FileRequestMessage
// #[derive(Deserialize, Serialize, Clone, Debug)]
// pub struct FileRequest {
//     pub request_type: String,
//     #[serde(flatten)]
//     pub common: FileRequestMessage,
// }

// #[derive(Deserialize, Serialize, Clone, Debug)]
// pub struct FileResponse {
//     pub request_type: String,
//     #[serde(flatten)]
//     pub common: FileResponseMessage,
// }
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

pub trait TryIntoRequest {
    type Request: DeserializeOwned + Serialize;

    fn into_typed_request<T: 'static>(self) -> Result<T, Box<dyn std::error::Error + Send + Sync>>;

    fn into_request(
        value: Value,
    ) -> Result<Self::Request, Box<dyn std::error::Error + Send + Sync>>;
}

// pub trait NodeTransportable {
//     async fn node_transport(
//         &self,
//         state: &AppState,
//         connection_handler: &mut ConnectionHandler,
//     ) -> Result<(), Box<dyn Error + Send + Sync>>;
// }

#[derive(Serialize)]
pub struct ServerDataResponse {
    pub state: GetState,
}
// impl IntoResponse<String> for ServerDataResponse {
//     // fn as_bytes(&self) -> Vec<u8> {
//     //     serde_json::to_vec(self).unwrap()
//     // }

//     // fn to_string(&self) -> Result<String, network_abstraction_lib::ExtractorErrors> {
//     //     serde_json::to_string(self)
//     //         .map_err(|_| network_abstraction_lib::ExtractorErrors::FailedToExtract)
//     // }
    
//     fn try_into_response(&self) -> Result<String, network_abstraction_lib::ExtractorErrors> {
//         serde_json::to_string(self)
//             .map_err(|_| network_abstraction_lib::ExtractorErrors::FailedToExtract)
//     }
// }

// impl NodeTransportable for ServerDataResponse {
//     async fn node_transport(
//         &self,
//         state: &AppState,
//         _connection_handler: &mut ConnectionHandler,
//     ) -> Result<(), Box<dyn Error + Send + Sync>> {
//         let tx = state.output_tx.lock().await.clone().unwrap();
//         let _ = tx.send(serde_json::to_string(&self.state)?).await;
//         Ok(())
//     }
// }

#[derive(Serialize)]
pub struct PingResponse {
    pub message: SimpleMessage,
}
// impl IntoResponse<String> for PingResponse {
//     // fn as_bytes(&self) -> Vec<u8> {
//     //     serde_json::to_vec(self).unwrap()
//     // }

//     // fn to_string(&self) -> Result<String, network_abstraction_lib::ExtractorErrors> {
//     //     serde_json::to_string(self)
//     //         .map_err(|_| network_abstraction_lib::ExtractorErrors::FailedToExtract)
//     // }
    
//     fn try_into_response(&self) -> Result<String, network_abstraction_lib::ExtractorErrors> {
//         serde_json::to_string(self)
//             .map_err(|_| network_abstraction_lib::ExtractorErrors::FailedToExtract)
//     }
// }

// impl NodeTransportable for PingResponse {
//     async fn node_transport(
//         &self,
//         state: &AppState,
//         _connection_handler: &mut ConnectionHandler,
//     ) -> Result<(), Box<dyn Error + Send + Sync>> {
//         let tx = state.output_tx.lock().await.clone().unwrap();
//         let _ = tx.send(serde_json::to_string(&self.message)?).await;
//         Ok(())
//     }
// }

#[derive(Debug)]
pub struct FileOperationResponse {
    pub data: String,
}
// impl NodeTransportable for FileOperationResponse {
//     async fn node_transport(
//         &self,
//         state: &AppState,
//         _connection_handler: &mut ConnectionHandler,
//     ) -> Result<(), Box<dyn Error + Send + Sync>> {
//         let tx = state.output_tx.lock().await.clone().unwrap();
//         let _ = tx.send(self.data.clone()).await;
//         Ok(())
//     }
// }

#[derive(Serialize)]
pub struct ServerNameResponse {
    pub message: MessagePayload,
}
// impl IntoResponse<String> for ServerNameResponse {
//     // fn as_bytes(&self) -> Vec<u8> {
//     //     serde_json::to_vec(self).unwrap()
//     // }

//     // fn to_string(&self) -> Result<String, network_abstraction_lib::ExtractorErrors> {
//     //     serde_json::to_string(self)
//     //         .map_err(|_| network_abstraction_lib::ExtractorErrors::FailedToExtract)
//     // }
    
//     fn try_into_response(&self) -> Result<String, network_abstraction_lib::ExtractorErrors> {
//         serde_json::to_string(self)
//             .map_err(|_| network_abstraction_lib::ExtractorErrors::FailedToExtract)
//     }
// }

// impl NodeTransportable for ServerNameResponse {
//     async fn node_transport(
//         &self,
//         state: &AppState,
//         _connection_handler: &mut ConnectionHandler,
//     ) -> Result<(), Box<dyn Error + Send + Sync>> {
//         let tx = state.output_tx.lock().await.clone().unwrap();
//         let _ = tx.send(serde_json::to_string(&self.message)?).await;
//         Ok(())
//     }
// }

#[derive(Serialize)]
pub struct ServerStateResponse {
    pub message: MessagePayload,
}
// impl IntoResponse<String> for ServerStateResponse {
//     // fn as_bytes(&self) -> Vec<u8> {
//     //     serde_json::to_vec(self).unwrap()
//     // }

//     // fn to_string(&self) -> Result<String, network_abstraction_lib::ExtractorErrors> {
//     //     serde_json::to_string(self)
//     //         .map_err(|_| network_abstraction_lib::ExtractorErrors::FailedToExtract)
//     // }
    
//     fn try_into_response(&self) -> Result<String, network_abstraction_lib::ExtractorErrors> {
//         serde_json::to_string(self)
//             .map_err(|_| network_abstraction_lib::ExtractorErrors::FailedToExtract)
//     }
// }

// impl NodeTransportable for ServerStateResponse {
//     async fn node_transport(
//         &self,
//         state: &AppState,
//         _connection_handler: &mut ConnectionHandler,
//     ) -> Result<(), Box<dyn Error + Send + Sync>> {
//         let tx = state.output_tx.lock().await.clone().unwrap();
//         let _ = tx.send(serde_json::to_string(&self.message)?).await;
//         Ok(())
//     }
// }

// pub struct RawBytes {
//     pub bytes: Vec<u8>
// }
// impl RawBytes {
//     async fn raw_transport(
//         &self,
//         // state: &AppState,
//         writer: &mut Writer,
//     ) -> Result<(), Box<dyn std::error::Error + Send + Sync>>{
//         writer.send(self.bytes.clone()).await
//     }
// }

// impl NodeTransportable for RawBytes {
//     async fn node_transport(
//         &self,
//         state: &AppState,
//         connection_handler: &mut ConnectionHandler,
//     ) -> Result<(), Box<dyn Error + Send + Sync>> {
//         let tx = state.output_tx.lock().await.clone().unwrap();
//         let _ = tx.send(self.bytes);
//         Ok(())
//     }
// }
