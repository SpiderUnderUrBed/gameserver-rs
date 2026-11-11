use serde::{Deserialize, Serialize};

use crate::database::databasespec::UserPerm;



#[derive(Deserialize, Serialize)]
pub struct FrontendNodeCreateRequest {
    pub nodename: String,
    pub ip: String
}

#[derive(Deserialize)]
pub struct FrontendChangeNodeRequest {
    pub node_id: String,
}

#[derive(Deserialize)]
pub struct FrontendUserCreateRequest {
    pub password: String,
    pub user: String,
    pub user_perms: Vec<UserPerm>,
}