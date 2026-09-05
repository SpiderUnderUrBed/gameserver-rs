use std::sync::{mpsc, Arc};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio_util::sync::CancellationToken;

use crate::{MetadataTypes, SrcAndDest, database::databasespec::Filters};
use tokio::sync::{broadcast, Notify};
pub struct ServernameRequest {
    pub ip: String,
}
pub struct DeleteServerRequest {
    pub metadata: MetadataTypes,
}
pub struct CreateServerRequest {
    pub metadata: MetadataTypes,
}

pub struct StartServerRequest {
    // metadata: MetadataTypes
    #[allow(unused)]
    pub stdin: Option<broadcast::Receiver<String>>,
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

pub struct ServerStateRequest {}

pub struct FileUploadRequest {
    pub(crate) stream: flume::Receiver<Vec<u8>>
}

pub struct FileDownloadRequest {
    pub(crate) stream: flume::Receiver<Vec<u8>>,
    pub(crate) task_end: Arc<CancellationToken>
}
