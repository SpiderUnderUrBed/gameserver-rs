use std::sync::{Arc};
use std::{any::Any, error::Error};
use futures::future::pending;
use tokio::sync::{broadcast, RwLock};

use crate::transport::node_transport::proto::server_manage_server::{ServerManage, ServerManageServer};
// use crate::transport::node_transport::proto::{};
use crate::MessagePayload;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use serde_json::Value;
use tcp_filesystem::FileRequestMessage;
use crate::{AppState, GetState, IncomingMessage, IncomingMessageWithMetadata, SimpleMessage};

use tonic::transport::{Server};
mod proto {
    tonic::include_proto!("main");
}
use proto::{server_edit_server::ServerEditServer, server_edit_server::ServerEdit};


pub struct ConnectionManager {
    url: String,
    accepted_connection: bool
}
impl ConnectionManager {
    pub async fn serve(url: String) -> Result<ConnectionManager, Box<dyn std::error::Error + Send + Sync>> {

        Ok(
            ConnectionManager { 
                accepted_connection: false,
                url
            }
        )
    }
    pub async fn accept_connection(
        &mut self,
    ) -> Result<(ConnectionHandler, Option<String>), Box<dyn std::error::Error + Send + Sync>> {
        if !self.accepted_connection {
            self.accepted_connection = true;
        } else {
            let _never: () = pending().await;
        }
        let connection = Connection {
                requests: Arc::new(RwLock::new(Vec::new())),
            };
        let handler = ConnectionHandler {
            // current_request: None,
            // requests: vec![],
            connection,
            read_buf: vec![],
            current_request: None,
        };
        let url = self.url.clone();
        let requests_lock = handler.connection.requests.clone();
        tokio::spawn(async move {
            //let mut requests = requests_lock.write().await;
            let inner_connection = Connection {
                requests: requests_lock,
            };
            let _ = Connection::serve_with_arc(Arc::new(inner_connection), url).await;
        });
        Ok((handler, None))
    }
}
#[derive(Default)]
pub struct Connection {
    requests: Arc<RwLock<Vec<Request>>>,
}
#[derive(Clone)]
pub struct Request {
    data: String,
    result_tx: broadcast::Sender<String>,
}


async fn delegate_request<T: Serialize, K: Serialize + DeserializeOwned + Clone>(
    request: T,
    requests_lock: Arc<RwLock<Vec<Request>>>,
) -> Result<Result<K, Box<dyn std::error::Error + Send + Sync>>, tonic::Status> {
    let (tx, mut rx) = broadcast::channel::<String>(16);

    println!("A");
    //println!("{:#?}", serde_json::to_value(request));
    let stringified_request = serde_json::to_string(&request)
        .map_err(|_| tonic::Status::internal(String::new()))?;

    let mut requests = requests_lock.write().await;
    requests.push(Request {
        data: stringified_request,
        result_tx: tx,
    });
    drop(requests); 
    println!("B");

    let result = rx
        .recv()
        .await
        .map_err(|_| tonic::Status::internal("Internal error".to_string()))?;
    println!("C");
    println!("{:#?}", result.clone());
    let serialized_result = serde_json::from_str::<K>(&result).map_err(|e| e.into());
    if let Err(e) = &serialized_result {
        println!("error: {:#?}", e);
    };
    Ok(serialized_result)
}

#[tonic::async_trait]
impl ServerEdit for Connection {
        async fn create(
            &self,
            request: tonic::Request<proto::CreateServerRequest>,
        ) -> std::result::Result<
            tonic::Response<proto::CreateServerResponse>,
            tonic::Status,
        >{
            //let inner = request.into_inner();
            let create_server_request = CreateServerRequest { 
                common: IncomingMessageWithMetadata { 
                    metadata: request.get_ref().clone().metadata.unwrap().into(),
                    message: "create_server".to_string(),
                    message_type: "command".to_string(),
                    authcode: "0".to_string(),
                }
            };
            
            match delegate_request::<CreateServerRequest, proto::CreateServerResponse>(create_server_request, self.requests.clone()).await {
                Ok(Ok(response)) => Ok(response.into()),
                Ok(Err(_)) => Err(tonic::Status::ok("done")),
                Err(_) => Err(tonic::Status::ok("done"))
            }
        }
        async fn delete(
            &self,
            request: tonic::Request<proto::DeleteServerRequest>,
        ) -> std::result::Result<
            tonic::Response<proto::DeleteServerResponse>,
            tonic::Status,
        >{
            //let inner = request.into_inner();
            let delete_server_request = DeleteServerRequest { 
                common: IncomingMessageWithMetadata { 
                    message: "delete_server".to_string(), 
                    message_type: "command".to_string(), 
                    metadata: request.get_ref().clone().metadata.unwrap().into(), 
                    authcode: "0".to_string() 
                }
            };
            match delegate_request::<DeleteServerRequest, proto::DeleteServerResponse>(delete_server_request, self.requests.clone()).await {
                Ok(Ok(response)) => Ok(response.into()),
                Ok(Err(_)) => Err(tonic::Status::ok("done")),
                Err(_) => Err(tonic::Status::ok("done"))
            }
        }
        async fn start(
            &self,
            request: tonic::Request<proto::StartServerRequest>,
        ) -> std::result::Result<
            tonic::Response<proto::StartServerResponse>,
            tonic::Status,
        >{
            //let inner = request.into_inner();
            let start_server_request = StartServerRequest::default();
            match delegate_request::<StartServerRequest, proto::StartServerResponse>(start_server_request, self.requests.clone()).await {
                Ok(Ok(response)) => Ok(response.into()),
                Ok(Err(_)) => Err(tonic::Status::ok("done")),
                Err(_) => Err(tonic::Status::ok("done"))
            }
        }
        async fn stop(
            &self,
            request: tonic::Request<proto::StopServerRequest>,
        ) -> std::result::Result<
            tonic::Response<proto::StopServerResponse>,
            tonic::Status,
        > {
            //let inner = request.into_inner();
            let stop_server_request = StopServerRequest::default();
            match delegate_request::<StopServerRequest, proto::StopServerResponse>(stop_server_request, self.requests.clone()).await {
                Ok(Ok(response)) => Ok(response.into()),
                Ok(Err(_)) => Err(tonic::Status::ok("done")),
                Err(_) => Err(tonic::Status::ok("done"))
            }
        }
}

#[tonic::async_trait]
impl ServerManage for Connection {
    async fn data(
        &self,
        request: tonic::Request<proto::ServerDataRequest>,
    ) -> std::result::Result<
        tonic::Response<proto::ServerDataResponse>,
        tonic::Status,
    > {
        let server_data_request = ServerDataRequest::default();
        match delegate_request::<ServerDataRequest, proto::ServerDataResponse>(server_data_request, self.requests.clone()).await {
            Ok(Ok(response)) => Ok(response.into()),
            Ok(Err(_)) => Err(tonic::Status::ok("done")),
            Err(_) => Err(tonic::Status::ok("done"))
        }
    }
    async fn name(
        &self,
        request: tonic::Request<proto::ServerNameRequest>,
    ) -> std::result::Result<
        tonic::Response<proto::ServerNameResponse>,
        tonic::Status,
    > {
         let server_name_request = ServerNameRequest::default();
        match delegate_request::<ServerNameRequest, proto::ServerNameResponse>(server_name_request, self.requests.clone()).await {
            Ok(Ok(response)) => 
            {
                println!("returning ok response");
                Ok(response.into())
            },
            Ok(Err(_)) => {
                println!("error 1");
                Err(tonic::Status::ok("done"))
            },
            Err(_) => {
                println!("Error 2");
                Err(tonic::Status::ok("done"))
            }
        }
    }
    async fn set(
        &self,
        request: tonic::Request<proto::SetServerRequest>,
    ) -> std::result::Result<
        tonic::Response<proto::SetServerResponse>,
        tonic::Status,
    > {
        let server_set_request = SetServerRequest { 
            common: IncomingMessageWithMetadata { 
                message: "set_server".to_string(), 
                message_type: "command".to_string(), 
                metadata: request.get_ref().metadata.clone().unwrap().into(), 
                authcode: "0".to_string() 
            }
        };
        match delegate_request::<SetServerRequest, proto::SetServerResponse>(server_set_request, self.requests.clone()).await {
            Ok(Ok(response)) => Ok(response.into()),
            Ok(Err(_)) => Err(tonic::Status::ok("done")),
            Err(_) => Err(tonic::Status::ok("done"))
        }
    }
    async fn state(
        &self,
        request: tonic::Request<proto::ServerStateRequest>,
    ) -> std::result::Result<
        tonic::Response<proto::ServerStateResponse>,
        tonic::Status,
    > {
        let server_state_request = ServerStateRequest::default();
        match delegate_request::<ServerStateRequest, proto::ServerStateResponse>(server_state_request, self.requests.clone()).await {
            Ok(Ok(response)) => Ok(response.into()),
            Ok(Err(_)) => Err(tonic::Status::ok("done")),
            Err(_) => Err(tonic::Status::ok("done"))
        }
    }
}

impl Into<crate::MetadataTypes> for proto::MetadataTypes {
    fn into(self) -> crate::MetadataTypes {
        serde_json::from_value(serde_json::to_value(self).unwrap()).unwrap()
    }
}

impl Connection {
    pub async fn serve_with_arc(self: Arc<Self>, url: String) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let addr = url.parse()?;
        Server::builder()
            .add_service(ServerEditServer::from_arc(self.clone()))  
            .add_service(ServerManageServer::from_arc(self))
            .serve(addr)
            .await?;
        Ok(())
    }
}
pub struct ConnectionHandler { 
    // current_request: Option<String>,
    // requests: Vec<String>,
    connection: Connection,
    //current_request: Option<String>,
    current_request: Option<Request>,
    read_buf: Vec<u8>
}

impl ConnectionHandler {
    pub fn new() -> ConnectionHandler {
        ConnectionHandler {
            // current_request: None,
            // requests: Vec::new(),
            connection: Connection {  
                    requests: Arc::new(RwLock::new(Vec::new()))
                },
            read_buf: Vec::new() ,
            current_request: None,
        }
    }

    pub fn inner(&mut self) -> &mut Vec<u8> {
        &mut self.read_buf
    }

    // pub fn serve(handler: &ConnectionHandler){

    // }

    pub fn clear(&self) {}
    pub async fn start_clean_hook(&mut self){

    }
    pub async fn end_clean_hook(&mut self){
        self.remove_current_segment_or_clear().await;
    }
    pub async fn remove_current_segment_or_clear(&mut self) {
        self.current_request = None;
        let mut requests = self.connection.requests.write().await;
        if requests.len() > 0 {
            // let _ = requests.get(0).unwrap().result_tx.send("\0".to_string());
            requests.remove(0);
        }
        // if let Some(pos) = requests.iter().position(predicate){
        // }
        //self.remove_segment_or_clear(self.newline_pos).await;
    }
    //pub fn next() -> Option<usize> {
    pub async fn next(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let requests = self.connection.requests.read().await;
        if requests.len() > 0 {
            let current_request = requests.get(0).unwrap();
            println!("have a current request");
            // let _ = current_request.result_tx.send("\0".to_string());
            self.current_request = Some(current_request.clone());
            Ok(())
        } else {
            Err("nothing in requests".into())
        }
        //todo!()
    }
    pub async fn recv_line(&mut self) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        if let Some(request) = &mut self.current_request {
            Ok(request.data.to_string())
        } else {
            Err("was nothing".into())
        }
    }
    pub async fn append_solution(&mut self, solution: String) {
        let _ = self.current_request.as_mut().unwrap().result_tx.send(solution);
    }
    pub async fn append_bytes(&mut self, bytes: Vec<u8>) {
        //self.inner().extend_from_slice(&bytes);
    }
    pub async fn has_remaining_buffer(&self) -> bool {
        false
    }
    pub async fn handle_request(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        todo!()
        //Ok(())
    }
    pub async fn send(
        &mut self,
        bytes: Vec<u8>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        todo!()
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
        Ok((Writer::default(), Reader::default()))
        //todo!()
    }
    // pub async fn handle_connections() {
    // }
}
#[derive(Default)]
pub struct Writer {
}
impl Writer {
    pub async fn send(
        &mut self,
        bytes: Vec<u8>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        Err("plain send is not supported for grpc".into())
    }
    pub async fn send_with_connection(
        &mut self,
        bytes: Vec<u8>,
        handler: &mut ConnectionHandler
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        
        Ok(())
    }
}
#[derive(Default)]
pub struct Reader {
    //read_buf: Option<&Vec<u8>>,
}
impl Reader {
    pub async fn recv(&mut self) -> Result<Vec<u8>, Box<dyn std::error::Error + Send + Sync>> {
        Err("receiving directly is not supported for grpc".into())
    }
    pub async fn handle_request(&mut self, handler: &mut ConnectionHandler) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        
        Ok(())
    }
} 

// TODO: work on a macro which leads to GRPC skipping the event loop
// the test the implimentation, find a way to directly connect the GRPC code here
// to the functional code defined in main
//inventory
macro_rules! connection {
    () => {
        
    };
}
macro_rules! register_request {
    ($arg1:ty => $arg2:ident) => {
        
    };
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
            println!("Got a file request");
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
impl Default for ServerStateRequest {
    fn default() -> Self {
        ServerStateRequest { 
            common: IncomingMessage { 
                message: "server_state".to_string(), 
                message_type: "command".to_string(), 
                authcode: "0".to_string()
            }
        }
    }
}

#[derive(Deserialize, Serialize, Clone)]
pub struct StopServerRequest {
    #[serde(flatten)]
    pub common: IncomingMessage,
}
impl Default for StopServerRequest {
    fn default() -> Self { 
        StopServerRequest { 
            common: IncomingMessage { 
                message: "stop_server".to_string(), 
                message_type: "command".to_string(), 
                authcode: "0".to_string() 
            } 
        }
    }
}
#[derive(Deserialize, Serialize, Clone)]
pub struct StartServerRequest {
    #[serde(flatten)]
    pub common: IncomingMessage,
}
impl Default for StartServerRequest {
    fn default() -> Self { 
        StartServerRequest { 
            common: IncomingMessage { 
                message: "start_server".to_string(), 
                message_type: "command".to_string(), 
                authcode: "0".to_string() 
            } 
        }
    }
}

#[derive(Deserialize, Serialize, Clone)]
pub struct ServerNameRequest {
    #[serde(flatten)]
    pub common: IncomingMessage,
}
impl Default for ServerNameRequest {
    fn default() -> Self { 
        ServerNameRequest { 
            common: IncomingMessage { 
                message: "server_name".to_string(), 
                message_type: "command".to_string(), 
                authcode: "0".to_string()
            } 
        }
    }
}

#[derive(Deserialize, Serialize, Clone)]
pub struct ServerDataRequest {
    #[serde(flatten)]
    pub common: IncomingMessage,
}
impl Default for ServerDataRequest {
    fn default() -> Self { 
        ServerDataRequest { 
            common: IncomingMessage { 
                message: "server_data".to_string(), 
                message_type: "command".to_string(), 
                authcode: "0".to_string()
            } 
        }
    }
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
    async fn node_transport(&self, state: &AppState, connection_handler: &mut ConnectionHandler) -> Result<(), Box<dyn Error + Send + Sync>>;
}

pub struct ServerDataResponse {
    pub state: GetState,
}
impl NodeTransportable for ServerDataResponse {
    async fn node_transport(&self, state: &AppState, connection_handler: &mut ConnectionHandler) -> Result<(), Box<dyn Error + Send + Sync>> {
        // let tx = state.output_tx.lock().await.clone().unwrap();
        // let _ = tx.send(serde_json::to_string(&self.state)?).await;
    connection_handler.append_solution(serde_json::to_string(&self.state)?).await;
        Ok(())
    }
}

pub struct PingResponse {
    pub message: SimpleMessage,
}
impl NodeTransportable for PingResponse {
    async fn node_transport(&self, state: &AppState, connection_handler: &mut ConnectionHandler) -> Result<(), Box<dyn Error + Send + Sync>> {
        // let tx = state.output_tx.lock().await.clone().unwrap();
        // let _ = tx.send(serde_json::to_string(&self.message)?).await;
        connection_handler.append_solution(serde_json::to_string(&self.message)?).await;
        Ok(())
    }
}

#[derive(Debug)]
pub struct FileOperationResponse {
    pub data: String,
}
impl NodeTransportable for FileOperationResponse {
    async fn node_transport(&self, state: &AppState, connection_handler: &mut ConnectionHandler) -> Result<(), Box<dyn Error + Send + Sync>> {
        // let tx = state.output_tx.lock().await.clone().unwrap();
        // let _ = tx.send(self.data.clone()).await;
        connection_handler.append_solution(self.data.clone()).await;
        Ok(())
    }
}

pub struct ServerNameResponse {
    pub message: MessagePayload,
}
impl NodeTransportable for ServerNameResponse {
    async fn node_transport(&self, state: &AppState, connection_handler: &mut ConnectionHandler) -> Result<(), Box<dyn Error + Send + Sync>> {
        // let tx = state.output_tx.lock().await.clone().unwrap();
        // let _ = tx.send(serde_json::to_string(&self.message)?).await;
        connection_handler.append_solution(serde_json::to_string(&self.message)?).await;
        Ok(())
    }
}

pub struct ServerStateResponse {
    pub message: MessagePayload,
}
impl NodeTransportable for ServerStateResponse {
    async fn node_transport(&self, state: &AppState, connection_handler: &mut ConnectionHandler) -> Result<(), Box<dyn Error + Send + Sync>> {
        // let tx = state.output_tx.lock().await.clone().unwrap();
        // let _ = tx.send(serde_json::to_string(&self.message)?).await;
        connection_handler.append_solution(serde_json::to_string(&self.message)?).await;
        Ok(())
    }
}
