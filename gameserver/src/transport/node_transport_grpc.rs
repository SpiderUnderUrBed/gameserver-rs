use std::{any::Any, error::Error};

use serde::{de::DeserializeOwned, Deserialize, Serialize};
use serde_json::Value;
use tcp_filesystem::{FileRequestMessage};
use crate::MessagePayload;

use crate::{AppState, GetState, IncomingMessage, IncomingMessageWithMetadata, SimpleMessage};





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
    pub state: GetState
}
impl NodeTransportable for ServerDataResponse {
    async fn node_transport(&self, state: &AppState) -> Result<(), Box<dyn Error + Send + Sync>> {
        let tx = state.output_tx.lock().await.clone().unwrap();
        let _ = tx.send(serde_json::to_string(&self.state)?).await;
        Ok(())    }
}

pub struct PingResponse {
    pub message: SimpleMessage
}
impl NodeTransportable for PingResponse {
    async fn node_transport(&self, state: &AppState) -> Result<(), Box<dyn Error + Send + Sync>> {
        let tx = state.output_tx.lock().await.clone().unwrap();
        let _ = tx.send(serde_json::to_string(&self.message)?).await;
        Ok(())
    }
}

pub struct FileOperationResponse {
    pub data: String
}
impl NodeTransportable for FileOperationResponse {
    async fn node_transport(&self, state: &AppState) -> Result<(), Box<dyn Error + Send + Sync>> {
        let tx = state.output_tx.lock().await.clone().unwrap();
        let _ = tx.send(self.data.clone()).await;
        Ok(())
    }
}

pub struct ServerNameResponse {
    pub message: MessagePayload
}
impl NodeTransportable for ServerNameResponse {
    async fn node_transport(&self, state: &AppState) -> Result<(), Box<dyn Error + Send + Sync>> {
        let tx = state.output_tx.lock().await.clone().unwrap();
        let _ = tx.send(serde_json::to_string(&self.message)?).await;
        Ok(())
    }
}

pub struct ServerStateResponse {
    pub message: MessagePayload
}
impl NodeTransportable for ServerStateResponse {
    async fn node_transport(&self, state: &AppState) -> Result<(), Box<dyn Error + Send + Sync>> {
        let tx = state.output_tx.lock().await.clone().unwrap();
        let _ = tx.send(serde_json::to_string(&self.message)?).await;
        Ok(())
    }
}