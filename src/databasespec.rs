use async_trait::async_trait;
// use sqlx::Error;
use std::fmt;
use std::error::Error;
use crate::Serialize;
use crate::Deserialize;
use crate::StatusCode;

use serde::Deserializer;

// use crate::NodeType;

use serde::ser::StdError;

#[cfg(any(feature = "full-stack", feature = "docker", feature = "database"))]
use sqlx::{postgres::{PgValueRef, PgArgumentBuffer}, Postgres, Type, Decode, Encode};

//use sqlx::error::BoxDynError;

use std::str::FromStr;
use futures_util::TryFutureExt;

type BoxDynError = Box<dyn StdError + Send + Sync>;

#[derive(Debug)]
pub struct DatabaseError(pub StatusCode);

impl fmt::Display for DatabaseError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "HTTP error: {}", self.0)
    }
}

impl Error for DatabaseError {}



// #[derive(Debug, Deserialize, Serialize, Clone)]
// pub struct RetrieveUser {
//     pub user: String
// }


#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct RetrieveElement {
    pub element: String
}



// #[derive(Debug, Deserialize, Serialize, Clone)]
// pub struct ModifyElementData {
//     pub element: String,
//     pub jwt: String,
//     pub(crate) require_auth: bool
// }

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
    Server(Server)
}
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct ModifyElementData {
    pub element: Element,
    // pub password: String,
    pub jwt: String,
    pub require_auth: bool,
//     pub user_perms: Vec<String>
}

#[cfg(all(not(feature = "full-stack"), not(feature = "database")))]
#[derive(Debug, Deserialize, Serialize, Clone, Default)]
pub struct Settings {
    pub(crate) toggled_default_buttons: bool
}

#[cfg(any(feature = "full-stack", feature = "database"))]
#[derive(Debug, Deserialize, Serialize, Clone, sqlx::FromRow, Default)]
pub struct Settings {
    pub toggled_default_buttons: bool
}


#[cfg(any(feature = "full-stack", feature = "database"))]
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, sqlx::Type)]
// #[sqlx(type_name = "node_status", rename_all = "snake_case")]
#[sqlx(type_name = "text")]
#[serde(rename_all = "snake_case", tag = "kind", content = "data")]
pub enum NodeStatus {
    Unknown,
    Enabled, 
    Disabled, 
    ImmutablyEnabled,
    ImmutablyDisabled,
}

use serde_json::Value;
// impl<'de> Deserialize<'de> for NodeStatus {
//     fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
//     where
//         D: Deserializer<'de>,
//     {
//         let raw = serde_json::Value::deserialize(deserializer)?;
//         println!("Raw JSON being parsed: {}", serde_json::to_string_pretty(&raw).unwrap_or_else(|_| format!("{:?}", raw)));
        
//         if let Some(s) = raw.as_str() {
//             let s = s.to_lowercase();
//             return Ok(match s.as_str() {
//                 "enabled" => NodeStatus::Enabled,
//                 "disabled" => NodeStatus::Disabled,
//                 "immutably_enabled" => NodeStatus::ImmutablyEnabled,
//                 "immutably_disabled" => NodeStatus::ImmutablyDisabled,
//                 _ => NodeStatus::Unknown,
//             });
//         }
        
//       
//         if let Some(obj) = raw.as_object() {
//             if let Some(state_val) = obj.get("state").and_then(|v| v.as_str()) {
//                 let s = state_val.to_lowercase();
//                 return Ok(match s.as_str() {
//                     "enabled" => NodeStatus::Enabled,
//                     "disabled" => NodeStatus::Disabled,
//                     "immutably_enabled" => NodeStatus::ImmutablyEnabled,
//                     "immutably_disabled" => NodeStatus::ImmutablyDisabled,
//                     _ => NodeStatus::Unknown,
//                 });
//             }
//         }
        
//         println!("Unknown format for NodeStatus: {:?}", raw);
//         Ok(NodeStatus::Unknown)
//     }
// }
#[cfg(all(
    not(feature = "full-stack"),
    not(feature = "docker"),
    not(feature = "database")
))]
#[derive(Debug, Serialize, Clone, Deserialize, PartialEq)]
// #[sqlx(type_name = "node_status", rename_all = "snake_case")]
#[serde(rename_all = "snake_case", tag = "kind", content = "data")]
pub enum NodeStatus {
    Unknown,
    Enabled, 
    Disabled, 
    ImmutablyEnabled,
    ImmutablyDisabled,
}
//`
#[derive(Debug, Deserialize, Serialize, Clone, PartialEq, Default)]
pub enum NodeType {
    #[default]
    Unknown,
    Custom,
    CustomWithString(String),
    InbuiltWithString(String),
    Inbuilt,
    Main
}


#[cfg(any(feature = "full-stack", feature = "docker", feature = "database"))]
impl Type<Postgres> for NodeType {
    fn type_info() -> sqlx::postgres::PgTypeInfo {
        <String as Type<Postgres>>::type_info()
    }
}

#[cfg(any(feature = "full-stack", feature = "docker", feature = "database"))]
impl<'r> Decode<'r, Postgres> for NodeType {
    fn decode(value: PgValueRef<'r>) -> Result<Self, BoxDynError> {
        let s = <String as Decode<Postgres>>::decode(value)?;
        Ok(NodeType::from(s))
    }
}


#[cfg(any(feature = "full-stack", feature = "docker", feature = "database"))]
impl<'q> Encode<'q, Postgres> for NodeType {
    fn encode_by_ref(
        &self,
        buf: &mut PgArgumentBuffer
    ) -> Result<sqlx::encode::IsNull, BoxDynError> {
        <String as Encode<Postgres>>::encode_by_ref(&self.to_string(), buf)
            .map_err(|e| Box::<dyn StdError + Send + Sync>::from(e))
    }

    fn size_hint(&self) -> usize {
        self.to_string().len()
    }
}


impl From<String> for NodeType {
    fn from(s: String) -> Self {
        match s.as_str() {
            "custom" => NodeType::Custom,
            "inbuilt" => NodeType::Inbuilt,
            "main" => NodeType::Main,
            other => NodeType::CustomWithString(other.to_string()),
        }
    }
}

impl ToString for NodeType {
    fn to_string(&self) -> String {
        match self {
            NodeType::Custom => "custom".to_string(),
            NodeType::Inbuilt => "inbuilt".to_string(),
            NodeType::Main => "main".to_string(),
            NodeType::CustomWithString(s) => s.clone(),
            NodeType::InbuiltWithString(s) => s.clone(),
	   _ => String::new()
        }
    }
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
    pub nodestatus: NodeStatus,
    pub nodetype: NodeType,
}


#[cfg(all(not(feature = "full-stack"), not(feature = "database")))]
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Node {
    pub nodename: String,
    pub ip: String,
    pub nodestatus: NodeStatus,
    pub nodetype: NodeType
}

#[cfg(any(feature = "full-stack", feature = "database"))]
#[derive(Clone, Debug, sqlx::FromRow, Serialize, Deserialize, PartialEq)]
pub struct Button {
    pub name: String,
    pub link: String,
    pub r#type: String
    //CustomType
}


#[cfg(all(not(feature = "full-stack"), not(feature = "database")))]
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Button {
    pub name: String,
    pub link: String,
    pub r#type: String
}


#[cfg(any(feature = "full-stack", feature = "database"))]
#[derive(Clone, Debug, sqlx::FromRow, Serialize, Deserialize)]
pub struct Server {
    pub servername: String,
    pub provider: String,
    pub providertype: String,
    pub location: String
}

#[cfg(all(not(feature = "full-stack"), not(feature = "database")))]
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Server {
    pub servername: String,
    pub provider: String,
    pub providertype: String,
    pub location: String
} 



// #[async_trait]
pub trait UserDatabase {
    async fn retrieve_user(&self, username: String) -> Option<User>;
    async fn fetch_all(&self) -> Result<Vec<User>, Box<dyn Error + Send + Sync>>;
    async fn get_from_database(&self, username: &str) -> Result<Option<User>, Box<dyn Error + Send + Sync>>;
    async fn create_user_in_db(&self, user: ModifyElementData) -> Result<StatusCode, Box<dyn Error + Send + Sync>>;
    async fn remove_user_in_db(&self, user: ModifyElementData) -> Result<StatusCode, Box<dyn Error + Send + Sync>>;
    async fn edit_user_in_db(&self, user: ModifyElementData) -> Result<StatusCode, Box<dyn Error + Send + Sync>>;
}

pub trait ServerDatabase {
    async fn retrieve_server(&self, servername: String) -> Option<Server>;
    async fn fetch_all_servers(&self) -> Result<Vec<Server>, Box<dyn Error + Send + Sync>>;
    async fn get_from_servers_database(&self, username: &str) -> Result<Option<Server>, Box<dyn Error + Send + Sync>>;
    async fn create_server_in_db(&self, server: ModifyElementData) -> Result<StatusCode, Box<dyn Error + Send + Sync>>;
    async fn remove_server_in_db(&self, server: ModifyElementData) -> Result<StatusCode, Box<dyn Error + Send + Sync>>;
    async fn edit_server_in_db(&self, server: ModifyElementData) -> Result<StatusCode, Box<dyn Error + Send + Sync>>;
}
pub trait NodesDatabase {
    async fn retrieve_nodes(&self, nodename: String) -> Option<Node>;
    async fn fetch_all_nodes(&self) -> Result<Vec<Node>, Box<dyn Error + Send + Sync>>;
    async fn get_from_nodes_database(&self, nodename: &str) -> Result<Option<Node>, Box<dyn Error + Send + Sync>>;
    async fn create_nodes_in_db(&self, node: ModifyElementData) -> Result<StatusCode, Box<dyn Error + Send + Sync>>;
    async fn remove_node_in_db(&self, node: ModifyElementData) -> Result<StatusCode, Box<dyn Error + Send + Sync>>;
    async fn edit_node_in_db(&self, node: ModifyElementData) -> Result<StatusCode, Box<dyn Error + Send + Sync>>;
}
pub trait ButtonsDatabase {
    async fn retrieve_buttons(&self, name: String) -> Option<Button>;
    async fn fetch_all_buttons(&self) -> Result<Vec<Button>, Box<dyn Error + Send + Sync>>;
    async fn toggle_default_buttons(&self) -> Result<StatusCode, Box<dyn Error + Send + Sync>>;
    async fn toggle_button_state(&self) -> Result<bool, Box<dyn Error + Send + Sync>>;
    async fn reset_buttons(&self) -> Result<StatusCode, Box<dyn Error + Send + Sync>>;
    async fn get_from_buttons_database(&self, name: &str) -> Result<Option<Button>, Box<dyn Error + Send + Sync>>;
    async fn edit_button_in_db(&self, button: ModifyElementData) -> Result<StatusCode, Box<dyn Error + Send + Sync>>;
}
