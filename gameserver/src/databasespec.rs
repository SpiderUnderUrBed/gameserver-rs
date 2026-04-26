use std::collections::HashMap;

use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Default, Clone)]
pub struct Database {
    pub current_server: String,
    pub server_index: HashMap<String, ServerIndex>,
}

#[derive(Clone, Default, Debug, Deserialize, Serialize)]
pub struct ServerIndex {
    pub(crate) location: String,
    pub(crate) provider: String,
    pub(crate) providertype: String,
    pub(crate) sandbox: bool,
}

impl ServerIndex {
    pub fn new(
        location: String,
        provider: String,
        providertype: String,
        sandbox: bool,
    ) -> ServerIndex {
        ServerIndex {
            location,
            provider,
            providertype,
            sandbox,
        }
    }
}
