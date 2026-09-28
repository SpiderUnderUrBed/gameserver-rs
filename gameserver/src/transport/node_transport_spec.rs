
use serde::{Deserialize, Serialize};

//#[cfg(feature = "grpc_experimental")]
use crate::{
    GetState, IncomingMessage, IncomingMessageWithMetadata, MessagePayload, SimpleMessage, Status,
};

use flatten_safe_macro::flatten_safe;

#[flatten_safe(tag_value = "server_state")]
#[derive(Serialize, Clone)]
pub struct ServerStateRequest {
    #[flatten]
    pub common: IncomingMessage,
    // pub state_action: StateActionType,
}


impl Default for ServerStateRequest {
    fn default() -> Self {
        ServerStateRequest {
            common: IncomingMessage {
                message: "server_state".to_string(),
                message_type: "command".to_string(),
                authcode: "0".to_string(),
            },
            // state_action: StateActionType::default(),
        }
    }
}

#[flatten_safe(tag_value = "server_state_updates")]
#[derive(Serialize, Clone)]
pub struct ServerStateUpdatesRequest {
    #[flatten]
    pub common: IncomingMessage,
    // pub state_action: StateActionType,
}

impl Default for ServerStateUpdatesRequest {
    fn default() -> Self {
        ServerStateUpdatesRequest {
            common: IncomingMessage {
                message: "server_state_updates".to_string(),
                message_type: "command".to_string(),
                authcode: "0".to_string(),
            },
            // state_action: StateActionType::default(),
        }
    }
}


#[flatten_safe(tag_value = "stop_server")]
#[derive(Serialize, Clone)]
pub struct StopServerRequest {
    #[flatten]
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

#[flatten_safe(tag_value = "server_name")]
#[derive(Serialize, Clone)]
pub struct ServerNameRequest {
    #[flatten]
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

#[flatten_safe(tag_value = "delete_server")]
#[derive(Serialize, Clone, Debug)]
pub struct DeleteServerRequest {
    #[flatten]
    pub common: IncomingMessageWithMetadata,
}

#[flatten_safe(tag_value = "set_server")]
#[derive(Serialize, Clone)]
pub struct SetServerRequest {
    #[flatten]
    pub common: IncomingMessageWithMetadata,
}
#[flatten_safe(tag_value = "set_filter")]
#[derive(Serialize, Clone)]
pub struct SetFilterRequest {
    #[flatten]
    pub common: IncomingMessageWithMetadata,
}

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
    pub server: String,
    pub channel: String,
}

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
    pub r#type: String,
    pub message: Status,
    pub authcode: String,
}


#[flatten_safe(tag_value = "create_server")]
#[derive(Serialize, Clone)]
pub struct CreateServerRequest {
    #[flatten]
    pub common: IncomingMessageWithMetadata,
}

#[flatten_safe(tag_value = "connect_server")]
#[derive(Serialize, Clone)]
pub struct ConnectServerRequest {
    #[flatten]
    pub common: IncomingMessage,
}

impl Default for ConnectServerRequest {
    fn default() -> Self {
        ConnectServerRequest {
            common: IncomingMessage {
                message: "connect_server".to_string(),
                message_type: "command".to_string(),
                authcode: "0".to_string(),
            },
        }
    }
}


#[flatten_safe(tag_value = "start_server")]
#[derive(Serialize, Clone)]
pub struct StartServerRequest {
    #[flatten]
    pub common: IncomingMessage,
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
