
use std::any::Any;

use serde::{Deserialize, Serialize};

//#[cfg(feature = "grpc_experimental")]
use crate::{MessagePayload, IncomingMessage, IncomingMessageWithMetadata, SimpleMessage, GetState, ValueRequest};
use network_abstraction_lib::{FromWire, IntoRequest};

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


impl IntoRequest for ConsoleRequest {
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
pub struct ServerStateRequest {
    #[serde(flatten)]
    pub common: IncomingMessage,
}

impl IntoRequest for ServerStateRequest {
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


impl FromWire for ServerStateRequest {
    type Request = ValueRequest;

    type Error = serde_json::Error;

    fn from_wire(req: Self::Request) -> Result<Self, Self::Error> {
        serde_json::from_value(req.value)
    }
}

impl Default for ServerStateRequest {
    fn default() -> Self {
        ServerStateRequest {
            common: IncomingMessage {
                message: "server_state".to_string(),
                message_type: "command".to_string(),
                authcode: "0".to_string(),
            },
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
                authcode: "0".to_string(),
            },
        }
    }
}

impl IntoRequest for StopServerRequest {
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


impl IntoRequest for ServerNameRequest {
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

impl FromWire for ServerNameRequest {
    type Request = ValueRequest;

    type Error = serde_json::Error;

    fn from_wire(req: Self::Request) -> Result<Self, Self::Error> {
        serde_json::from_value(req.value)
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

impl FromWire for ServerDataRequest {
    type Request = ValueRequest;

    type Error = serde_json::Error;

    fn from_wire(req: Self::Request) -> Result<Self, Self::Error> {
        serde_json::from_value(req.value)
    }
}


impl IntoRequest for ServerDataRequest {
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
pub struct DeleteServerRequest {
    #[serde(flatten)]
    pub common: IncomingMessageWithMetadata,
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


impl IntoRequest for SetServerRequest {
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


impl IntoRequest for SetFilterRequest {
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

impl IntoRequest for Ping {
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
    #[serde(flatten)]
    pub common: MessagePayload,
}

#[derive(Serialize)]
pub struct ServerStateResponse {
    pub message: MessagePayload,
}