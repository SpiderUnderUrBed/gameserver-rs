use std::{error::Error, sync::{Arc, mpsc}};

use arc_swap::ArcSwap;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio_util::sync::CancellationToken;

use crate::{AppState, MetadataTypes, SrcAndDest, database::databasespec::Filters, transport::node_transport::ConnectionHandler};
use tokio::sync::{Notify, RwLock, broadcast};
pub struct ServernameRequest {
    pub ip: String,
}
pub struct DeleteServerRequest {
    pub metadata: MetadataTypes,
}

pub struct SwitchConsoleRequest {
    pub active: CancellationToken,
    pub stdin: broadcast::Receiver<String>,
    pub stdout: broadcast::Sender<String>
}

pub struct CreateServerRequest {
    pub active: CancellationToken,
    pub metadata: MetadataTypes,
}

pub struct StartServerRequest {
    // metadata: MetadataTypes
    pub active: CancellationToken,
    #[allow(unused)]
    pub stdin: broadcast::Receiver<String>,
    pub stdout: broadcast::Sender<String>
}
pub struct StopServerRequest {
    // metadata: MetadataTypes
}
pub struct CapabilitiesRequest {
    pub capabilities: Vec<String>,
}

#[derive(Serialize, Deserialize)]
pub struct MigrateRequest {
    #[serde(flatten)]
    pub common: SrcAndDest,
}

pub struct SetServerRequest {
    pub(crate) metadata: MetadataTypes,
}

pub struct ServerDataRequest {
    pub(crate) metadata: MetadataTypes,
}

pub struct FilterRequest {
    //pub(crate) //metadata: MetadataTypes
    pub(crate) filter: Filters,
}

pub struct Ping {}

//InternalTransportable
pub struct IntegrationKeyRequest {
    pub key: Value,
}


#[derive(Deserialize, Serialize, Clone, Default)]
pub enum StateActionType {
    #[default]
    Immediate,
    OnUpdate
}

pub struct ServerStateRequest {
    // pub(crate) state_action: StateActionType
}

pub struct RemoteFile {
    pub location: String,
    pub stream: Option<flume::Receiver<Vec<u8>>>
}

pub struct FileUploadRequest {
    pub(crate) file: RemoteFile,
}

// impl FileUploadRequest {
//     pub fn new(stream: impl Into<RemoteFile>) -> Self {
//         Self { stream: stream.into() }
//     }
// }

// pub struct FileUploadRequest {
//     pub(crate) stream: flume::Receiver<Vec<u8>>
// }

pub struct FileDownloadRequest {
    pub(crate) file: RemoteFile,
    pub(crate) task_end: Arc<CancellationToken>
}

pub trait NodeTransportable {
    type Output;
    async fn node_transport(&self, state: &AppState) -> Result<Self::Output, Box<dyn Error + Send + Sync>>;
}

pub trait NodeTransportableMut {
    type Output;
    async fn node_transport(&self, state: &mut AppState) -> Result<Self::Output, Box<dyn Error + Send + Sync>>;
}

pub trait CustomNodeTransportable {
    type Output;
    async fn custom_node_transport(&self, state: &AppState, connection_handler: &ConnectionHandler) -> Result<Self::Output, Box<dyn Error + Send + Sync>>;
}

pub trait StreamTransportable {
    type Output;
    async fn stream_transport(
        &self,
        state: Arc<ArcSwap<AppState>>,
    ) -> Result<Self::Output, Box<dyn Error + Send + Sync>>;
}
