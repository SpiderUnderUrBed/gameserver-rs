use futures::future::pending;
use futures::Stream;
use futures::StreamExt;
use general_networked_filesystem::core::DirectoryResponse;
use general_networked_filesystem::core::FileOperations;
use general_networked_filesystem::core::FileRequestExecutable;
use general_networked_filesystem::core::SizeResponse;
use std::fs::File;
use std::io::BufReader;
use std::io::Read;
use std::io::Write;
use std::pin::Pin;
use std::sync::Arc;
use tokio::sync::{broadcast, mpsc};
use tonic::Streaming;

use crate::create_server_handler;
use crate::start_server_handler;
use crate::SimpleMessage;
use crate::transport::node_transport::proto::filesystem_manage_server::FilesystemManageServer;
use crate::transport::node_transport::proto::node_manage_server::NodeManageServer;
use crate::transport::node_transport::proto::CanonicalizeResponse;
use crate::transport::node_transport::proto::DeleteServerResponse;
use crate::transport::node_transport::proto::FileChunk;
use crate::transport::node_transport::proto::FileItem;
use crate::transport::node_transport::proto::LsRequest;
use crate::transport::node_transport::proto::RawFileChunk;
use crate::transport::node_transport::proto::ServerMessage;
use crate::transport::node_transport::proto::SetServerResponse;
use crate::transport::node_transport::proto::StopServerResponse;
use crate::transport::node_transport::proto::UploadResponse;
use crate::transport::node_transport_spec::ServerDataResponse;
use crate::transport::node_transport_spec::ServerStateResponse;
use crate::GetState;
use crate::MessagePayload;
use crate::{AppState, IncomingMessage, IncomingMessageWithMetadata};
use tokio::sync::Mutex;
use tokio_stream::wrappers::ReceiverStream;
use tonic::transport::Server;

use crate::transport::node_transport_spec::ConsoleRequest;
use crate::transport::node_transport_spec::CreateServerRequest;
use crate::transport::node_transport_spec::DeleteServerRequest;
use crate::transport::node_transport_spec::ServerDataRequest;
use crate::transport::node_transport_spec::ServerNameRequest;
use crate::transport::node_transport_spec::ServerStateRequest;
use crate::transport::node_transport_spec::SetServerRequest;
use crate::transport::node_transport_spec::StartServerRequest;
use crate::transport::node_transport_spec::StopServerRequest;
mod proto {
    tonic::include_proto!("main");
}
use crate::transport::node_transport::proto::filesystem_manage_server::FilesystemManage;
use crate::transport::node_transport::proto::node_manage_server::NodeManage;
use crate::transport::node_transport::proto::server_manage_server::{
    ServerManage, ServerManageServer,
};
use proto::{server_edit_server::ServerEdit, server_edit_server::ServerEditServer};


use crate::{
    console_handler, delete_server_handler, server_data_handler, server_name_handler,
    server_state_handler, set_filter_handler, set_server_handler, stop_server_handler,
};

pub struct ClientManager {

}
impl ClientManager {
    pub fn new() -> ClientManager {
        ClientManager {}
    }
}

pub enum BackgroundTaskUpdates {
    NoMoreFileTransfer,
}

pub async fn spawn_conn_background_tasks(
    arc_state: Arc<AppState>,
    arc_conn_manager: Arc<Mutex<ConnectionManager>>,
) {
}
pub struct ConnectionManager {
    url: String,
    accepted_connection: bool,
    state: Arc<AppState>,
}
impl ConnectionManager {
    pub async fn serve(
        state: Arc<AppState>,
        url: String,
    ) -> Result<ConnectionManager, Box<dyn std::error::Error + Send + Sync>> {
        Ok(ConnectionManager {
            accepted_connection: false,
            url,
            state,
        })
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
            state: self.state.clone(),
        };
        let handler = ConnectionHandler {
            connection: connection.clone(),
        };

        let inner_url = self.url.clone();
        tokio::spawn(async move {
            let _ = Connection::serve_with_arc(Arc::new(connection), inner_url).await;
        });
        Ok((handler, None))
    }
}

#[derive(Clone)]
pub struct Connection {
    state: Arc<AppState>,
}
#[derive(Clone)]
pub struct Request {
    data: String,
    result_tx: broadcast::Sender<String>,
}

#[tonic::async_trait]
impl ServerEdit for Connection {
    type StartStream = ReceiverStream<Result<ServerMessage, tonic::Status>>;
    type CreateStream = ReceiverStream<Result<ServerMessage, tonic::Status>>;
    async fn create(
        &self,
        request: tonic::Request<proto::CreateServerRequest>,
    ) -> std::result::Result<tonic::Response<Self::CreateStream>, tonic::Status> {
        // //let inner = request.into_inner();
        let (tx, rx) = mpsc::channel(32);

        let create_server_request = CreateServerRequest {
            common: IncomingMessageWithMetadata {
                metadata: request.get_ref().clone().metadata.unwrap().into(),
                message: "create_server".to_string(),
                message_type: "command".to_string(),
                authcode: "0".to_string(),
            },
        };
        
        let raw_stream = create_server_handler(&self.state.clone(), create_server_request, "unknown".into()).await
                        .map_err(|_| tonic::Status::internal("failed to create server"))?;

        tokio::spawn(async move {
            let mut raw_stream_guard = raw_stream.inner.lock().await;
            let stream = raw_stream_guard.as_mut().unwrap();
            while let Some(message) = stream.next().await {
                tx.send(Ok(
                    ServerMessage { authcode:"0".into(), data: message,  message: "console".into(), channel: "stdout".into(), servername: "unknown".into() }
                )).await;
            }
        });

        Ok(tonic::Response::new(ReceiverStream::new(rx)))
    }
    async fn delete(
        &self,
        request: tonic::Request<proto::DeleteServerRequest>,
    ) -> std::result::Result<tonic::Response<proto::DeleteServerResponse>, tonic::Status> {
        let inner = request.into_inner();
        let delete_server_request = DeleteServerRequest {
            common: IncomingMessageWithMetadata {
                message: "delete_server".to_string(),
                message_type: "command".to_string(),
                metadata: inner.clone().metadata.unwrap().into(),
                authcode: "0".to_string(),
            },
        };

        delete_server_handler(&self.state.clone(), delete_server_request).await;

        Ok(tonic::Response::new(proto::DeleteServerResponse {}))
    }
    async fn start(
        &self,
        // request: tonic::Request<proto::StartServerRequest>,
        request: tonic::Request<Streaming<proto::ServerMessage>>,
    ) -> std::result::Result<tonic::Response<Self::CreateStream>, tonic::Status> {
        let start_server_request = StartServerRequest::default();

        let (tx, rx) = mpsc::channel(32);

        let mut inbound = request.into_inner();
        let inner_state = self.state.clone();
        tokio::spawn(async move {
            while let Some(result) = inbound.next().await {
                match result {
                    Ok(message) => {
                        console_handler(
                            &inner_state, 
                            message.into(),
                            "unknown".into()
                        ).await;
                    }
                    Err(_) => {
                        println!("got an error in the stream");
                    }
                }
            }
        });


        let raw_stream = start_server_handler(&self.state.clone(), start_server_request, "unknown".into()).await
                        .map_err(|_| tonic::Status::internal("error starting server"))?;
        tokio::spawn(async move {
            let mut raw_stream_guard = raw_stream.inner.lock().await;
            let mut stream = raw_stream_guard.as_mut().unwrap();
            while let Some(message) = stream.next().await {
                tx.send(Ok(
                    ServerMessage { 
                        authcode: "0".into(), 
                        data: message, 
                        message: "console".into(), 
                        channel: "stdout".into(), 
                        servername: "unknown".into() 
                    }
                )).await;
            }
        });

        println!("returning a stream");
        Ok(tonic::Response::new(ReceiverStream::new(rx)))
    }
    async fn stop(
        &self,
        _request: tonic::Request<proto::StopServerRequest>,
    ) -> std::result::Result<tonic::Response<proto::StopServerResponse>, tonic::Status> {
        //let inner = request.into_inner();
        let stop_server_request = StopServerRequest::default();

        stop_server_handler(&self.state.clone(), stop_server_request, "unknown".into()).await
            .map_err(|_| tonic::Status::internal("There is no server to stop"))?;

        Ok(tonic::Response::new(StopServerResponse {}))
    }
}

#[tonic::async_trait]
impl ServerManage for Connection {
    async fn data(
        &self,
        _request: tonic::Request<proto::ServerDataRequest>,
    ) -> std::result::Result<tonic::Response<proto::ServerDataResponse>, tonic::Status> {
        let server_data_request = ServerDataRequest::default();

        let server_data_result = server_data_handler(&self.state.clone(), server_data_request, "unknown".into()).await;

        if let Ok(server_data_response) = server_data_result {
            Ok(tonic::Response::new(server_data_response.into()))
        } else {
            Err(tonic::Status::internal("Failed to get the server data"))
        }
    }

    async fn set(
        &self,
        request: tonic::Request<proto::SetServerRequest>,
    ) -> std::result::Result<tonic::Response<proto::SetServerResponse>, tonic::Status> {
        let server_set_request = SetServerRequest {
            common: IncomingMessageWithMetadata {
                message: "set_server".to_string(),
                message_type: "command".to_string(),
                metadata: request.get_ref().metadata.clone().unwrap().into(),
                authcode: "0".to_string(),
            },
        };

        
        set_server_handler(&self.state.clone(), server_set_request, "unknown".into()).await;

        Ok(tonic::Response::new(SetServerResponse {}))
    }
    async fn state(
        &self,
        _request: tonic::Request<proto::ServerStateRequest>,
    ) -> std::result::Result<tonic::Response<proto::ServerStateResponse>, tonic::Status> {
        let server_state_request = ServerStateRequest::default();

        let state_response = server_state_handler(&self.state.clone(), server_state_request, "unknown".into()).await
                                .map_err(|_| tonic::Status::internal("Error getting the server state"))?;

        Ok(tonic::Response::new(state_response.into()))
    }
}

#[tonic::async_trait]
impl NodeManage for Connection {
    async fn name(
        &self,
        _request: tonic::Request<proto::ServerNameRequest>,
    ) -> std::result::Result<tonic::Response<proto::ServerNameResponse>, tonic::Status> {
        let server_name_request = ServerNameRequest::default();

        let resp = server_name_handler(&self.state.clone(), server_name_request, "unknown".into()).await;

        Ok(tonic::Response::new(proto::ServerNameResponse {
            r#type: resp.common.r#type,
            message: resp.common.message,
            authcode: resp.common.authcode,
        }))
    }
}

#[tonic::async_trait]
impl FilesystemManage for Connection {
    type DownloadStream = ReceiverStream<Result<RawFileChunk, tonic::Status>>;
    async fn ls(
        &self,
        request: tonic::Request<proto::LsRequest>,
    ) -> std::result::Result<tonic::Response<proto::LsResponse>, tonic::Status> {
        // let state = self.router.lock().await.get_state_mut();
        let fs_request = general_networked_filesystem::core::LsRequest::from_proto(request);
        let resp = fs_request
            .execute()
            .map_err(|e| tonic::Status::internal("Error executing file request"))?;
        Ok(resp.into_tonic_response())
    }
    async fn canonicalize(
        &self,
        request: tonic::Request<proto::CanonicalizeRequest>,
    ) -> std::result::Result<tonic::Response<proto::CanonicalizeResponse>, tonic::Status> {
        let fs_request =
            general_networked_filesystem::core::CannonolizeRequest::from_proto(request);
        let resp = fs_request
            .execute()
            .map_err(|e| tonic::Status::internal("Error executing file request"))?;
        Ok(resp.into_tonic_response())
    }
    async fn size(
        &self,
        request: tonic::Request<proto::SizeRequest>,
    ) -> std::result::Result<tonic::Response<proto::SizeResponse>, tonic::Status> {
        let fs_request = general_networked_filesystem::core::SizeRequest::from_proto(request);
        let resp = fs_request
            .execute()
            .map_err(|e| tonic::Status::internal("Error executing file request"))?;
        Ok(resp.into_tonic_response())
    }
    async fn upload(
        &self,
        request: tonic::Request<Streaming<FileChunk>>,
    ) -> std::result::Result<tonic::Response<proto::UploadResponse>, tonic::Status> {
        println!("got an upload request");
        let mut inbound = request.into_inner();
        let mut location = String::new();
        let mut file_handle_option = None;
        while let Some(chunk_res) = inbound.next().await {
            if let Ok(chunk) = chunk_res {
                if location != chunk.location {
                    location = chunk.location;
                    if let Ok(file) = File::open(&location) {
                        file_handle_option = Some(file);
                    } else {
                        match File::create(&location) {
                            Ok(file) => file_handle_option = Some(file),
                            Err(e) => {
                                return Err(tonic::Status::internal(format!(
                                    "failed to create file at location with: {}",
                                    e
                                )))
                            }
                        };
                    }
                }
                if let Some(ref mut file_handle) = file_handle_option {
                    file_handle.write_all(&chunk.bytes).map_err(|e| {
                        tonic::Status::internal(format!(
                            "failed to write file at location with: {}",
                            e
                        ))
                    })?;
                    let _ = file_handle.flush();
                    let _ = file_handle.sync_all();
                }
            } else {
                return Err(tonic::Status::internal("Error streaming the file chunks"));
            }
        }
        Ok(UploadResponse {}.into())
    }
    async fn download(
        &self,
        request: tonic::Request<proto::DownloadRequest>,
    ) -> std::result::Result<tonic::Response<Self::DownloadStream>, tonic::Status> {
        let (tx, rx) = mpsc::channel(32);
        let location = &request.get_ref().location;
        let file = File::open(location).map_err(|e| {
            tonic::Status::internal(format!("Got an error opening the file: {}", e))
        })?;

        let mut reader = BufReader::new(file);
        let mut chunk = vec![0u8; 1000];
        tokio::spawn(async move {
            loop {
                let n = reader.read(&mut chunk);
                match n {
                    Ok(0) => {
                        break;
                    }
                    Ok(n) => {
                        let file_chunk = RawFileChunk {
                            bytes: chunk[..n].to_vec(),
                        };
                        if let Err(_) = tx.send(Ok(file_chunk)).await {
                            break;
                        }
                    }
                    Err(e) => {
                        println!("read error: {:#?}", e);
                        break;
                    }
                }
            }
        });
        Ok(tonic::Response::new(ReceiverStream::new(rx)))
    }
}

impl Into<crate::MetadataTypes> for proto::MetadataTypes {
    fn into(self) -> crate::MetadataTypes {
        // TODO: remove hardcoding of server for
        // metadata conversion?
        match self.kind.as_str() {
            "Server" => serde_json::from_str::<crate::MetadataTypes>(&self.data).unwrap(),
            _ => {
                let value = serde_json::to_value(self.clone()).unwrap();
                serde_json::from_value(value).unwrap()
            }
        }
    }
}
impl Connection {
    pub async fn serve_with_arc(
        self: Arc<Self>,
        url: String,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        println!("serving");
        let addr = url.parse()?;
        Server::builder()
            .add_service(ServerEditServer::from_arc(self.clone()))
            .add_service(ServerManageServer::from_arc(self.clone()))
            .add_service(NodeManageServer::from_arc(self.clone()))
            .add_service(FilesystemManageServer::from_arc(self))
            .serve(addr)
            .await?;
        Ok(())
    }
}

impl Into<ConsoleRequest> for proto::ServerMessage {
    fn into(self) -> ConsoleRequest {
        ConsoleRequest {
            common: SimpleMessage {
                message: "console".into(),
            },
            data: self.data,
            server: self.servername,
            channel: self.channel,
        }
    }
}

impl Into<proto::ServerStateResponse> for ServerStateResponse {
    fn into(self) -> proto::ServerStateResponse {
        proto::ServerStateResponse {
            message: Some(self.message.into()),
        }
    }
}
impl Into<proto::ServerDataResponse> for ServerDataResponse {
    fn into(self) -> proto::ServerDataResponse {
        proto::ServerDataResponse {
            state: Some(self.state.into()),
        }
    }
}
impl Into<proto::MessagePayload> for MessagePayload {
    fn into(self) -> proto::MessagePayload {
        proto::MessagePayload {
            r#type: self.r#type,
            message: self.message,
            authcode: self.authcode,
        }
    }
}
impl Into<proto::State> for GetState {
    fn into(self) -> proto::State {
        proto::State {
            name: self.name,
            start_keyword: self.start_keyword,
            stop_keyword: self.stop_keyword,
        }
    }
}

// TODO: consider manually mapping it in grpc routes rather than trait conversions
trait FromProto<T> {
    fn from_proto(value: T) -> Self;
}
trait IntoTonicResponse<T> {
    fn into_tonic_response(self) -> tonic::Response<T>;
}
impl FromProto<tonic::Request<proto::LsRequest>> for general_networked_filesystem::core::LsRequest {
    fn from_proto(value: tonic::Request<proto::LsRequest>) -> Self {
        general_networked_filesystem::core::LsRequest {
            id: 0,
            location: value.get_ref().location.clone(),
        }
    }
}
impl IntoTonicResponse<proto::LsResponse> for DirectoryResponse {
    fn into_tonic_response(self) -> tonic::Response<proto::LsResponse> {
        tonic::Response::new(proto::LsResponse {
            file_item: self
                .directory
                .iter()
                .map(|fs_item| FileItem {
                    name: fs_item.name.clone(),
                    is_dir: fs_item.is_dir,
                })
                .collect(),
        })
    }
}
impl FromProto<tonic::Request<proto::CanonicalizeRequest>>
    for general_networked_filesystem::core::CannonolizeRequest
{
    fn from_proto(value: tonic::Request<proto::CanonicalizeRequest>) -> Self {
        general_networked_filesystem::core::CannonolizeRequest {
            id: 0,
            path: value.get_ref().path.clone(),
        }
    }
}
impl IntoTonicResponse<proto::CanonicalizeResponse>
    for general_networked_filesystem::core::CannonolizeResponse
{
    fn into_tonic_response(self) -> tonic::Response<proto::CanonicalizeResponse> {
        tonic::Response::new(proto::CanonicalizeResponse {
            full_path: self.path,
        })
    }
}

impl FromProto<tonic::Request<proto::SizeRequest>>
    for general_networked_filesystem::core::SizeRequest
{
    fn from_proto(value: tonic::Request<proto::SizeRequest>) -> Self {
        general_networked_filesystem::core::SizeRequest {
            id: 0,
            location: value.get_ref().location.clone(),
        }
    }
}
impl IntoTonicResponse<proto::SizeResponse> for SizeResponse {
    fn into_tonic_response(self) -> tonic::Response<proto::SizeResponse> {
        tonic::Response::new(proto::SizeResponse { size: self.size })
    }
}

pub struct ConnectionHandler {
    // current_request: Option<String>,
    // requests: Vec<String>,
    connection: Connection,
}

impl ConnectionHandler {
    pub fn new(state: Arc<AppState>) -> ConnectionHandler {
        ConnectionHandler {
            // current_request: None,
            // requests: Vec::new(),
            connection: Connection { state },
        }
    }
}
