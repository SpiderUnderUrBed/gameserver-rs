use serde::{Deserialize, Serialize};



#[derive(Deserialize, Serialize)]
pub struct FrontendNodeCreateRequest {
    pub nodename: String,
    pub ip: String
}

#[derive(Deserialize)]
pub struct FrontendChangeNodeRequest {
    pub node_id: String,
}