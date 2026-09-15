use std::{any::Any, sync::Arc};

use serde::{Deserialize, Serialize};

//#[cfg(feature = "grpc_experimental")]
use crate::{
    AppState, GetState, IncomingMessage, IncomingMessageWithMetadata, MessagePayload, SimpleMessage, ValueRequest,
};
use network_abstraction_lib::{ErrorResponse, FromWire, IntoRequest, NoneResponse, register_output_single, typed::RouteInput, typed_request_macros::{self, register_output}, typed_stream::StreamRouteInput};


#[derive(Deserialize, Serialize, Clone)]
pub struct ConsoleRequest {
    authcode: String,
    pub(crate) data: String,
    server: String,
    channel: String,
    r#type: String,
}

#[typed_request_macros::typed_request(name = "console")]
impl RouteInput<Arc<AppState>> for ConsoleRequest {
    type Output = NoneResponse;
}

#[derive(Deserialize, Serialize, Clone, Default)]
pub enum StateActionType {
    #[default]
    Immediate,
    OnUpdate
}

#[derive(Deserialize, Serialize, Clone)]
pub struct ServerStateRequest {
    #[serde(flatten)]
    pub common: IncomingMessage,
    pub state_action: StateActionType
}

#[typed_request_macros::typed_request(snake_case)]
impl RouteInput<Arc<AppState>> for ServerStateRequest {
    type Output = ServerStateResponse;
}



impl Default for ServerStateRequest {
    fn default() -> Self {
        ServerStateRequest {
            common: IncomingMessage {
                message: "server_state".to_string(),
                message_type: "command".to_string(),
                authcode: "0".to_string(),
            },
            state_action: StateActionType::default(),
        }
    }
}

#[derive(Deserialize, Serialize, Clone)]
pub struct StopServerRequest {
    #[serde(flatten)]
    pub common: IncomingMessage,
}

#[typed_request_macros::typed_request(snake_case)]
impl RouteInput<Arc<AppState>> for StopServerRequest {
    type Output = Result<NoneResponse, ErrorResponse>;
}

impl Default for StopServerRequest {
    fn default() -> Self {
        StopServerRequest {
            common: IncomingMessage {
                message: "stop_server".to_string(),
                message_type: "command".to_string(),
                authcode: "0".to_string(),
            },
        }
    }
}

#[derive(Deserialize, Serialize, Clone)]
pub struct ServerNameRequest {
    #[serde(flatten)]
    pub common: IncomingMessage,
}

#[typed_request_macros::typed_request(snake_case)]
impl RouteInput<Arc<AppState>> for ServerNameRequest {
    type Output = ServerNameResponse;
}

impl Default for ServerNameRequest {
    fn default() -> Self {
        ServerNameRequest {
            common: IncomingMessage {
                message: "server_name".to_string(),
                message_type: "command".to_string(),
                authcode: "0".to_string(),
            },
        }
    }
}

impl Default for ServerDataRequest {
    fn default() -> Self {
        ServerDataRequest {
            common: IncomingMessage {
                message: "server_data".to_string(),
                message_type: "command".to_string(),
                authcode: "0".to_string(),
            },
        }
    }
}

#[derive(Deserialize, Serialize, Clone)]
pub struct ServerDataRequest {
    #[serde(flatten)]
    pub common: IncomingMessage,
}


#[typed_request_macros::typed_request(snake_case)]
impl RouteInput<Arc<AppState>> for ServerDataRequest {
    type Output = Result<ServerDataResponse, NoneResponse>; 
}



#[derive(Deserialize, Serialize, Clone, Debug)]
pub struct DeleteServerRequest {
    #[serde(flatten)]
    pub common: IncomingMessageWithMetadata,
}
// register_output!(DeleteServerRequest, "DeleteServerRequest");

#[typed_request_macros::typed_request(snake_case)]
impl RouteInput<Arc<AppState>> for DeleteServerRequest {
    type Output = NoneResponse; 
}

impl IntoRequest for DeleteServerRequest {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn into_any(self: Box<Self>) -> Box<dyn Any> {
        self
    }
    fn clone_box(&self) -> Box<dyn IntoRequest> {
        Box::new(self.clone())
    }
}


#[derive(Deserialize, Serialize, Clone)]
pub struct SetServerRequest {
    #[serde(flatten)]
    pub common: IncomingMessageWithMetadata,
}
#[typed_request_macros::typed_request(snake_case)]
impl RouteInput<Arc<AppState>> for SetServerRequest {
    type Output = NoneResponse; 
}


#[derive(Deserialize, Serialize, Clone)]
pub struct SetFilterRequest {
    #[serde(flatten)]
    pub common: IncomingMessageWithMetadata,
}

#[typed_request_macros::typed_request(snake_case)]
impl RouteInput<Arc<AppState>> for SetFilterRequest {
    type Output = NoneResponse; 
}


#[derive(Deserialize, Serialize, Default, Clone)]
pub struct Ping {
    #[serde(flatten)]
    pub common: SimpleMessage,
}

#[typed_request_macros::typed_request(snake_case)]
impl RouteInput<Arc<AppState>> for Ping {
    type Output = PingResponse; 
}

#[register_output]
#[derive(Serialize, Clone, Debug, Deserialize)]
pub struct ServerDataResponse {
    pub state: GetState,
}

#[register_output]
#[derive(Serialize, Clone, Debug, Deserialize)]
pub struct PingResponse {
    pub message: SimpleMessage,
}

#[register_output]
#[derive(Serialize, Clone, Debug, Deserialize)]
pub struct ServerNameResponse {
    #[serde(flatten)]
    pub common: MessagePayload,
}
// register_output_single!(ServerNameResponse);

#[register_output]
#[derive(Serialize, Clone, Debug, Deserialize)]
pub struct ServerStateResponse {
    pub message: MessagePayload,
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

impl IntoRequest for CreateServerRequest {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn into_any(self: Box<Self>) -> Box<dyn Any> {
        self
    }
    fn clone_box(&self) -> Box<dyn IntoRequest> {
        Box::new(self.clone())
    }
}

#[derive(Deserialize, Serialize, Clone)]
pub struct StartServerRequest {
    #[serde(flatten)]
    pub common: IncomingMessage,
}
#[typed_request_macros::typed_stream_request(snake_case)]
impl StreamRouteInput<Arc<AppState>> for StartServerRequest {
    type Item = String;
}

impl IntoRequest for StartServerRequest {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn into_any(self: Box<Self>) -> Box<dyn Any> {
        self
    }
    fn clone_box(&self) -> Box<dyn IntoRequest> {
        Box::new(self.clone())
    }
}
impl FromWire for StartServerRequest {
    type Request = ValueRequest;

    type Error = serde_json::Error;

    fn from_wire(req: Self::Request) -> Result<Self, Self::Error> {
        serde_json::from_value(req.value)
    }
}

impl Default for StartServerRequest {
    fn default() -> Self {
        StartServerRequest {
            common: IncomingMessage {
                message: "start_server".to_string(),
                message_type: "command".to_string(),
                authcode: "0".to_string(),
            },
        }
    }
}