use std::{any::Any, sync::Arc};

use serde::{Deserialize, Serialize};

//#[cfg(feature = "grpc_experimental")]
use crate::{
    AppState, GetState, IncomingMessage, IncomingMessageWithMetadata, MessagePayload, SimpleMessage,
};

use flatten_safe_macro::flatten_safe;

// #[typed_request_macros::typed_request(name = "console")]
// impl RouteInput<Arc<AppState>> for ConsoleRequest {
//     type Output = NoneResponse;
// }

#[derive(Deserialize, Serialize, Clone, Default)]
pub enum StateActionType {
    #[default]
    Immediate,
    OnUpdate,
}

#[flatten_safe(tag_value = "server_state")]
#[derive(Serialize, Clone)]
pub struct ServerStateRequest {
    #[flatten]
    pub common: IncomingMessage,
    pub state_action: StateActionType,
}

// #[typed_request_macros::typed_request(snake_case)]
// impl RouteInput<Arc<AppState>> for ServerStateRequest {
//     type Output = ServerStateResponse;
// }

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

#[flatten_safe(tag_value = "stop_server")]
#[derive(Serialize, Clone)]
pub struct StopServerRequest {
    #[flatten]
    pub common: IncomingMessage,
}

// #[typed_request_macros::typed_request(snake_case)]
// impl RouteInput<Arc<AppState>> for StopServerRequest {
//     type Output = Result<NoneResponse, ErrorResponse>;
// }

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

#[flatten_safe(tag_value = "server_name")]
#[derive(Serialize, Clone)]
pub struct ServerNameRequest {
    #[flatten]
    pub common: IncomingMessage,
}

// #[typed_request_macros::typed_request(snake_case)]
// impl RouteInput<Arc<AppState>> for ServerNameRequest {
//     type Output = ServerNameResponse;
// }

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

#[flatten_safe(tag_value = "server_data")]
#[derive(Serialize, Clone)]
pub struct ServerDataRequest {
    #[flatten]
    pub common: IncomingMessage,
}

// #[typed_request_macros::typed_request(snake_case)]
// impl RouteInput<Arc<AppState>> for ServerDataRequest {
//     type Output = Result<ServerDataResponse, NoneResponse>;
// }

#[flatten_safe(tag_value = "delete_server")]
#[derive(Serialize, Clone, Debug)]
pub struct DeleteServerRequest {
    #[flatten]
    pub common: IncomingMessageWithMetadata,
}
// register_output!(DeleteServerRequest, "DeleteServerRequest");

// #[typed_request_macros::typed_request(snake_case)]
// impl RouteInput<Arc<AppState>> for DeleteServerRequest {
//     type Output = NoneResponse;
// }

// impl IntoRequest for DeleteServerRequest {
//     fn as_any(&self) -> &dyn Any {
//         self
//     }

//     fn into_any(self: Box<Self>) -> Box<dyn Any> {
//         self
//     }
//     fn clone_box(&self) -> Box<dyn IntoRequest> {
//         Box::new(self.clone())
//     }
// }

#[flatten_safe(tag_value = "set_server")]
#[derive(Serialize, Clone)]
pub struct SetServerRequest {
    #[flatten]
    pub common: IncomingMessageWithMetadata,
}
// #[typed_request_macros::typed_request(snake_case)]
// impl RouteInput<Arc<AppState>> for SetServerRequest {
//     type Output = NoneResponse;
// }

#[flatten_safe(tag_value = "set_filter")]
#[derive(Serialize, Clone)]
pub struct SetFilterRequest {
    #[flatten]
    pub common: IncomingMessageWithMetadata,
}

// #[typed_request_macros::typed_request(snake_case)]
// impl RouteInput<Arc<AppState>> for SetFilterRequest {
//     type Output = NoneResponse;
// }

#[flatten_safe(tag_value = "ping")]
#[derive(Serialize, Default, Clone)]
pub struct Ping {
    #[flatten]
    pub common: SimpleMessage,
}

#[flatten_safe(tag_value = "console")]
#[derive(Serialize, Clone)]
pub struct ConsoleRequest {
    #[flatten]
    pub(crate) common: SimpleMessage,
    pub(crate) data: String,
    server: String,
    channel: String,
}

// #[typed_request_macros::typed_request(snake_case)]
// impl RouteInput<Arc<AppState>> for Ping {
//     type Output = PingResponse;
// }

// #[register_output]
#[derive(Serialize, Clone, Debug, Deserialize)]
pub struct ServerDataResponse {
    pub state: GetState,
}

// #[register_output]
#[derive(Serialize, Clone, Debug, Deserialize)]
pub struct PingResponse {
    pub message: SimpleMessage,
}

// #[register_output]
#[flatten_safe]
#[derive(Serialize, Clone, Debug)]
pub struct ServerNameResponse {
    #[flatten]
    pub common: MessagePayload,
}
// register_output_single!(ServerNameResponse);

// #[register_output]
#[derive(Serialize, Clone, Debug, Deserialize)]
pub struct ServerStateResponse {
    pub message: MessagePayload,
}

#[flatten_safe]
#[derive(Serialize, Clone)]
pub struct CreateServerRequest {
    #[flatten]
    pub common: IncomingMessageWithMetadata,
}

// impl FromWire for CreateServerRequest {
//     type Request = ValueRequest;

//     type Error = serde_json::Error;

//     fn from_wire(req: Self::Request) -> Result<Self, Self::Error> {
//         serde_json::from_value(req.value)
//     }
// }

// impl IntoRequest for CreateServerRequest {
//     fn as_any(&self) -> &dyn Any {
//         self
//     }

//     fn into_any(self: Box<Self>) -> Box<dyn Any> {
//         self
//     }
//     fn clone_box(&self) -> Box<dyn IntoRequest> {
//         Box::new(self.clone())
//     }
// }

#[flatten_safe(tag_value = "start_server")]
#[derive(Serialize, Clone)]
pub struct StartServerRequest {
    #[flatten]
    pub common: IncomingMessage,
}
// #[typed_request_macros::typed_stream_request(snake_case)]
// impl StreamRouteInput<Arc<AppState>> for StartServerRequest {
//     type Item = String;
// }

// impl IntoRequest for StartServerRequest {
//     fn as_any(&self) -> &dyn Any {
//         self
//     }

//     fn into_any(self: Box<Self>) -> Box<dyn Any> {
//         self
//     }
//     fn clone_box(&self) -> Box<dyn IntoRequest> {
//         Box::new(self.clone())
//     }
// }
// impl FromWire for StartServerRequest {
//     type Request = ValueRequest;

//     type Error = serde_json::Error;

//     fn from_wire(req: Self::Request) -> Result<Self, Self::Error> {
//         serde_json::from_value(req.value)
//     }
// }

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
