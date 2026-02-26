use serde::{Deserialize, Serialize};

#[derive(Clone, Default, Debug, Deserialize, Serialize)]
pub struct ServerIndex {
    pub(crate) location: String,
    pub(crate) provider: String
}

impl ServerIndex {
    pub fn new(location: String, provider: String) -> ServerIndex {
        ServerIndex {
            location,
            provider
        }
    }
}