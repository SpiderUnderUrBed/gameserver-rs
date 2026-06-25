use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{
    AppState, MessagePayload, MessagePayloadWithMetadata, MetadataTypes, SimpleMessage, SrcAndDest,
    database::databasespec::Filters,
};
use std::error::Error;

pub trait NodeTransportable {
    async fn node_transport(&self, state: &AppState) -> Result<(), Box<dyn Error + Send + Sync>>;
}
pub struct DeleteServerRequest {
    pub metadata: MetadataTypes,
}
// NodeTransportable
impl NodeTransportable for DeleteServerRequest {
    async fn node_transport(&self, state: &AppState) -> Result<(), Box<dyn Error + Send + Sync>> {
        let msg = MessagePayloadWithMetadata {
            r#type: "command".to_string(),
            message: "delete_server".to_string(),
            authcode: "0".to_string(),
            metadata: self.metadata.clone(),
        };

        let mut bytes = match serde_json::to_vec(&msg) {
            Ok(b) => b,
            Err(e) => {
                eprintln!("Serialization error: {}", e);
                return Err("Failed to serialize".into());
            }
        };
        bytes.push(b'\n');

        let _ = state.tcp_tx.send(bytes);

        Ok(())
    }
}

pub struct CreateServerRequest {
    pub metadata: MetadataTypes,
}
impl NodeTransportable for CreateServerRequest {
    async fn node_transport(&self, state: &AppState) -> Result<(), Box<dyn Error + Send + Sync>> {
        let msg = MessagePayloadWithMetadata {
            r#type: "command".to_string(),
            message: "create_server".to_string(),
            metadata: self.metadata.clone(),
            authcode: "0".to_string(),
        };
        let mut bytes = match serde_json::to_vec(&msg) {
            Ok(b) => b,
            Err(e) => {
                eprintln!("Serialization error: {}", e);
                return Err("Failed to serialize".into());
            }
        };
        bytes.push(b'\n');
        let _ = state.tcp_tx.send(bytes);

        Ok(())
    }
}
// NodeTransportable

pub struct StartServerRequest {
    // metadata: MetadataTypes
}
impl NodeTransportable for StartServerRequest {
    async fn node_transport(&self, state: &AppState) -> Result<(), Box<dyn Error + Send + Sync>> {
        let msg = serde_json::to_vec(&MessagePayload {
            r#type: "command".to_string(),
            message: "start_server".to_string(),
            authcode: "".to_string(),
        });
        if let Err(e) = msg {
            return Err("Failed to serialize".into());
        };
        let _ = state.tcp_tx.send(msg.unwrap());

        Ok(())
    }
}

pub struct StopServerRequest {
    // metadata: MetadataTypes
}
impl NodeTransportable for StopServerRequest {
    async fn node_transport(&self, state: &AppState) -> Result<(), Box<dyn Error + Send + Sync>> {
        let msg = serde_json::to_vec(&MessagePayload {
            r#type: "command".to_string(),
            message: "stop_server".to_string(),
            authcode: "".to_string(),
        });
        if let Err(e) = msg {
            return Err("Failed to serialize".into());
        };
        let _ = state.tcp_tx.send(msg.unwrap());

        Ok(())
    }
}

#[derive(Serialize, Deserialize)]
pub struct MigrateRequest {
    #[serde(flatten)]
    pub common: SrcAndDest,
}
impl NodeTransportable for MigrateRequest {
    async fn node_transport(&self, state: &AppState) -> Result<(), Box<dyn Error + Send + Sync>> {
        match serde_json::to_vec(&self.common) {
            Ok(bytes) => {
                if let Err(err) = state.tcp_tx.send(bytes) {
                    eprintln!("Failed to send request over broadcast: {}", err);
                }
            }
            Err(err) => eprintln!("Failed to serialize request: {}", err),
        }

        Ok(())
    }
}

pub struct SetServerRequest {
    pub(crate) metadata: MetadataTypes,
}
impl NodeTransportable for SetServerRequest {
    async fn node_transport(&self, state: &AppState) -> Result<(), Box<dyn Error + Send + Sync>> {
        let msg = MessagePayloadWithMetadata {
            r#type: "command".to_string(),
            message: "set_server".to_string(),
            metadata: self.metadata.clone(),
            authcode: "0".to_string(),
        };
        let mut bytes = match serde_json::to_vec(&msg) {
            Ok(b) => b,
            Err(e) => {
                eprintln!("Serialization error: {}", e);
                return Err("Failed to serialize".into());
            }
        };
        bytes.push(b'\n');
        let _ = state.tcp_tx.send(bytes);

        Ok(())
    }
}
// NodeTransportable

pub struct ServerDataRequest {
    pub(crate) metadata: MetadataTypes,
}
impl NodeTransportable for ServerDataRequest {
    async fn node_transport(&self, state: &AppState) -> Result<(), Box<dyn Error + Send + Sync>> {
        let msg = MessagePayloadWithMetadata {
            r#type: "command".to_string(),
            message: "server_data".to_string(),
            metadata: self.metadata.clone(),
            authcode: "0".to_string(),
        };
        let mut bytes = match serde_json::to_vec(&msg) {
            Ok(b) => b,
            Err(e) => {
                eprintln!("Serialization error: {}", e);
                return Err("Failed to serialize".into());
            }
        };
        bytes.push(b'\n');
        let _ = state.tcp_tx.send(bytes);

        Ok(())
    }
}

// struct ServerState
// NodeTransportable

pub struct RawBytes {
    pub(crate) bytes: Vec<u8>,
}

impl NodeTransportable for RawBytes {
    async fn node_transport(&self, state: &AppState) -> Result<(), Box<dyn Error + Send + Sync>> {
        let _ = state.tcp_tx.send(self.bytes.clone());
        Ok(())
    }
}
// NodeTransportable

trait InternalTransportable {
    async fn internal_transport(
        &self,
        state: &AppState,
    ) -> Result<(), Box<dyn Error + Send + Sync>>;
}
pub struct FilterRequest {
    //pub(crate) //metadata: MetadataTypes
    pub(crate) filter: Filters,
}
//InternalTransportable
// struct FilterRequest impl NodeTransportable {
impl NodeTransportable for FilterRequest {
    async fn node_transport(&self, state: &AppState) -> Result<(), Box<dyn Error + Send + Sync>> {
        let filter_request = MessagePayloadWithMetadata {
            r#type: "command".to_string(),
            message: "set_filter".to_string(),
            metadata: MetadataTypes::Filter(self.filter.clone()),
            authcode: "0".to_string(),
        };
        let _ = state
            .tcp_tx
            .send(serde_json::to_vec(&filter_request).unwrap());

        Ok(())
    }
}
impl InternalTransportable for FilterRequest {
    async fn internal_transport(
        &self,
        state: &AppState,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        Ok(())
    }
}

// }
pub struct Ping {}
impl NodeTransportable for Ping {
    async fn node_transport(&self, state: &AppState) -> Result<(), Box<dyn Error + Send + Sync>> {
        let ping = SimpleMessage {
            message: "ping".to_string(),
        };
        let res = state.tcp_tx.send(serde_json::to_vec(&ping).unwrap());

        Ok(())
    }
}
impl InternalTransportable for Ping {
    async fn internal_transport(
        &self,
        state: &AppState,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        Ok(())
    }
}
//InternalTransportable
pub struct IntegrationKeyRequest {
    pub key: Value,
}
impl NodeTransportable for IntegrationKeyRequest {
    async fn node_transport(&self, state: &AppState) -> Result<(), Box<dyn Error + Send + Sync>> {
        match serde_json::to_vec(&self.key) {
            Ok(mut bytes) => {
                // Add newline delimiter for TCP stream parsing
                bytes.push(b'\n');

                if let Err(err) = state.tcp_tx.send(bytes.clone()) {
                    eprintln!("Failed to send to internal stream: {}", err);
                }

                // Tells the remote server to enable RCON
                //if let Some(internal_tx) = &state.internal_tx {
                if let Err(err) = state.tcp_tx.send(bytes) {
                    eprintln!("Failed to send to TCP stream: {}", err);
                }
                //}
            }
            Err(err) => eprintln!("Failed to serialize request: {}", err),
        }

        Ok(())
    }
}
impl InternalTransportable for IntegrationKeyRequest {
    async fn internal_transport(
        &self,
        state: &AppState,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        Ok(())
    }
}
//InternalTransportable
pub struct ServerStateRequest {}
impl NodeTransportable for ServerStateRequest {
    async fn node_transport(&self, state: &AppState) -> Result<(), Box<dyn Error + Send + Sync>> {
        let msg = serde_json::to_vec(&MessagePayload {
            r#type: "command".to_string(),
            message: "server_state".to_string(),
            authcode: "0".to_string(),
        })
        .unwrap();
        let _ = state.tcp_tx.send(msg);

        Ok(())
    }
}
impl InternalTransportable for ServerStateRequest {
    async fn internal_transport(
        &self,
        state: &AppState,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        Ok(())
    }
}
// NoteTransportable
//InternalTransportable
// struct SrcAndDestFsRequest {
//     src: String,
//     dest: String
// }
//InternalTransportable
// async fn send_request_to_node(){
// }
