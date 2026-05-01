use std::collections::HashMap;

use file_transfer_system::server::Server;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Default, Clone)]
pub struct Database {
    pub current_server: String,
    pub server_index: HashMap<String, ServerIndex>,
}

#[derive(Clone, Default, Debug, Deserialize, Serialize)]
pub struct ServerMetadata {
    start_keyword: Option<String>,
    stop_keyword: Option<String>
}

#[derive(Clone, Default, Debug, Deserialize, Serialize)]
pub struct ServerIndex {
    pub(crate) location: String,
    pub(crate) provider: String,
    pub(crate) providertype: String,
    pub(crate) sandbox: bool,
    pub(crate) server_metadata: ServerMetadata
}

impl ServerIndex {
    pub fn new(
        location: String,
        provider: String,
        providertype: String,
        sandbox: bool,
        server_metadata: ServerMetadata
    ) -> ServerIndex {
        ServerIndex {
            location,
            provider,
            providertype,
            sandbox,
            server_metadata
        }
    }
}
