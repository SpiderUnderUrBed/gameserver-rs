use async_trait::async_trait;
// use sqlx::Error;
use std::fmt;
use std::error::Error;
use crate::Serialize;
use crate::Deserialize;
use crate::StatusCode;

#[derive(Debug)]
pub struct DatabaseError(pub StatusCode);

impl fmt::Display for DatabaseError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "HTTP error: {}", self.0)
    }
}

impl Error for DatabaseError {}



#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct RetrieveUser {
    pub user: String
}


#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct RemoveElementData {
    pub element: String,
    pub jwt: String
}

#[cfg(any(feature = "full-stack", feature = "database"))]
#[derive(Debug, Deserialize, Serialize, Clone, sqlx::FromRow)]
#[serde(tag = "kind", content = "data")]
pub enum CustomType {
    Custom, 
    Default
}

#[cfg(all(not(feature = "full-stack"), not(feature = "database")))]
#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(tag = "kind", content = "data")]
pub enum CustomType {
    Custom, 
    Default
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(tag = "kind", content = "data")]
pub enum Element {
    User {  
        password: String,
        user: String,
        user_perms: Vec<String>
    },
    Node(Node),
    Button(Button),
    Server
}
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct CreateElementData {
    pub element: Element,
    // pub password: String,
    pub jwt: String,
    pub require_auth: bool,
//     pub user_perms: Vec<String>
}

#[cfg(any(feature = "full-stack", feature = "database"))]
#[derive(Clone, Debug, sqlx::FromRow, Serialize, Deserialize)]
pub struct User {
    pub username: String,
    pub password_hash: Option<String>,
    pub user_perms: Vec<String>
}

#[cfg(all(not(feature = "full-stack"), not(feature = "database")))]
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct User {
    pub username: String,
    pub password_hash: Option<String>,
    pub user_perms: Vec<String>
}

#[cfg(any(feature = "full-stack", feature = "database"))]
#[derive(Clone, Debug, sqlx::FromRow, Serialize, Deserialize, PartialEq)]
pub struct Node {
    pub nodename: String,
    pub ip: String,
    pub nodetype: String
}

#[cfg(all(not(feature = "full-stack"), not(feature = "database")))]
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Node {
    pub nodename: String,
    pub ip: String,
    pub nodetype: String
}

#[cfg(any(feature = "full-stack", feature = "database"))]
#[derive(Clone, Debug, sqlx::FromRow, Serialize, Deserialize, PartialEq)]
pub struct Button {
    pub name: String,
    pub link: String,
    pub r#type: CustomType
}


#[cfg(all(not(feature = "full-stack"), not(feature = "database")))]
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Button {
    pub name: String,
    pub link: String,
    pub r#type: CustomType
}


#[cfg(any(feature = "full-stack", feature = "database"))]
#[derive(Clone, Debug, sqlx::FromRow, Serialize, Deserialize)]
pub struct Server {
    pub servername: String,
}

#[cfg(all(not(feature = "full-stack"), not(feature = "database")))]
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Server {
    pub servername: String,
}


// #[async_trait]
pub trait UserDatabase {
    async fn retrieve_user(&self, username: String) -> Option<User>;
    async fn fetch_all(&self) -> Result<Vec<User>, Box<dyn Error + Send + Sync>>;
    async fn get_from_database(&self, username: &str) -> Result<Option<User>, Box<dyn Error + Send + Sync>>;
    async fn create_user_in_db(&self, user: CreateElementData) -> Result<StatusCode, Box<dyn Error + Send + Sync>>;
    async fn remove_user_in_db(&self, user: RemoveElementData) -> Result<StatusCode, Box<dyn Error + Send + Sync>>;
    async fn edit_user_in_db(&self, user: CreateElementData) -> Result<StatusCode, Box<dyn Error + Send + Sync>>;
}

pub trait ServerDatabase {
    async fn retrieve_server(&self, username: String) -> Option<User>;
    async fn fetch_all_servers(&self) -> Result<Vec<User>, Box<dyn Error + Send + Sync>>;
    async fn get_from_servers_database(&self, username: &str) -> Result<Option<User>, Box<dyn Error + Send + Sync>>;
    async fn create_server_in_db(&self, user: CreateElementData) -> Result<StatusCode, Box<dyn Error + Send + Sync>>;
    async fn remove_server_in_db(&self, user: RemoveElementData) -> Result<StatusCode, Box<dyn Error + Send + Sync>>;
    async fn edit_server_in_db(&self, user: CreateElementData) -> Result<StatusCode, Box<dyn Error + Send + Sync>>;
}
pub trait NodesDatabase {
    async fn retrieve_nodes(&self, nodename: String) -> Option<Node>;
    async fn fetch_all_nodes(&self) -> Result<Vec<Node>, Box<dyn Error + Send + Sync>>;
    async fn get_from_nodes_database(&self, nodename: &str) -> Result<Option<Node>, Box<dyn Error + Send + Sync>>;
    async fn create_nodes_in_db(&self, node: CreateElementData) -> Result<StatusCode, Box<dyn Error + Send + Sync>>;
    async fn remove_node_in_db(&self, node: RemoveElementData) -> Result<StatusCode, Box<dyn Error + Send + Sync>>;
    async fn edit_node_in_db(&self, node: CreateElementData) -> Result<StatusCode, Box<dyn Error + Send + Sync>>;
}
pub trait ButtonsDatabase {
    async fn retrieve_buttons(&self, name: String) -> Option<Button>;
    async fn fetch_all_buttons(&self) -> Result<Vec<Button>, Box<dyn Error + Send + Sync>>;
    async fn get_from_buttons_database(&self, name: &str) -> Result<Option<Button>, Box<dyn Error + Send + Sync>>;
    async fn edit_button_in_db(&self, button: CreateElementData) -> Result<StatusCode, Box<dyn Error + Send + Sync>>;
}