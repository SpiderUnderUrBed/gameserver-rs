use std::collections::HashMap;

use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Default, Clone)]
pub struct Database {
    pub settings: Settings,
    pub current_server: String,
    pub filter: Filters,
    pub server_index: HashMap<String, ServerIndex>,
}

#[derive(Debug, Serialize, Deserialize, Default, Clone)]
#[serde(rename_all = "snake_case", tag = "kind", content = "data")]
pub enum Filters {
    AlternatingLine,
    #[default]
    None,
}

#[derive(Debug, Serialize, Deserialize, Default, Clone)]
pub struct Settings {
    pub pre_hook_timeout: Option<u64>,
    pub install_hook_timeout: Option<u64>,
    pub post_hook_timeout: Option<u64>,
    pub process_timeout: Option<u64>,
}

#[derive(Clone, Default, Debug, Deserialize, Serialize)]
pub struct ServerMetadata {
    start_keyword: Option<String>,
    stop_keyword: Option<String>,
}

#[derive(Clone, Default, Debug, Deserialize, Serialize)]
pub struct ServerIndex {
    pub(crate) location: String,
    pub(crate) provider: String,
    pub(crate) providertype: String,
    pub(crate) sandbox: bool,
    pub(crate) server_metadata: ServerMetadata,
}

impl ServerIndex {
    pub fn new(
        location: String,
        provider: String,
        providertype: String,
        sandbox: bool,
        server_metadata: ServerMetadata,
    ) -> ServerIndex {
        ServerIndex {
            location,
            provider,
            providertype,
            sandbox,
            server_metadata,
        }
    }
}
