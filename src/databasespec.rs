use async_trait::async_trait;
#[cfg(any(feature = "full-stack", feature = "database"))]
use std::default;
#[cfg(any(feature = "full-stack", feature = "database"))]
use std::default;
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


// #[derive(Debug, Deserialize, Serialize, Clone)]
// pub struct RetrieveElement {
//     pub element: Element,
//     pub jwt: String,
//     pub require_auth: bool
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
    Server(Server),
    Intergration(Intergration),
    String(String)
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
// #[derive(Debug, Deserialize, Serialize, Clone, Default)]
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Settings {
    pub(crate) toggled_default_buttons: bool,
    pub(crate) status_type: String,
    pub(crate) enabled_rcon: bool,
    pub(crate) rcon_url: String,
    pub(crate) rcon_password: String,
    pub(crate) driver: String,
    pub(crate) file_system_driver: String,
    pub(crate) enable_statistics_on_home_page: String,
    pub(crate) current_server: Server
}

impl Default for Settings {
    fn default() -> Self {
        Self { 
            toggled_default_buttons: Default::default(), 
            status_type: Default::default(), 
            enabled_rcon: true, 
            rcon_url: "localhost:25575".to_string(), 
            rcon_password: "testing".to_string(),
            driver: "".to_string(),
            enable_statistics_on_home_page: "".to_string(),
            file_system_driver: "".to_string(),
            current_server: Server::default(),
        }
    }
}

#[cfg(any(feature = "full-stack", feature = "database"))]
#[derive(Debug, Deserialize, Serialize, Clone, sqlx::FromRow)]
pub struct Settings {
    pub(crate) toggled_default_buttons: bool,
    pub(crate) status_type: String,
    pub(crate) enabled_rcon: bool,
    pub(crate) rcon_url: String,
    pub(crate) rcon_password: String,
    pub(crate) driver: String,
    pub(crate) file_system_driver: String,
    pub(crate) enable_statistics_on_home_page: String,
    pub(crate) current_server: String,
}


#[cfg(any(feature = "full-stack", feature = "database"))]
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, sqlx::Type, Default)]
// #[sqlx(type_name = "node_status", rename_all = "snake_case")]
#[sqlx(type_name = "text")]
#[serde(rename_all = "snake_case", tag = "kind", content = "data")]
pub enum NodeStatus {
    #[default]
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
#[derive(Debug, Serialize, Clone, Deserialize, PartialEq, Default)]
// #[sqlx(type_name = "node_status", rename_all = "snake_case")]
#[serde(rename_all = "snake_case", tag = "kind", content = "data")]
pub enum NodeStatus {
    #[default]
    Unknown,
    Enabled, 
    Disabled, 
    ImmutablyEnabled,
    ImmutablyDisabled,
}
//
#[cfg(all(not(feature = "full-stack"), not(feature = "database")))]
#[derive(Debug, Deserialize, Serialize, Clone, PartialEq, Default)]
pub enum K8sType {
    Node,
    Pod,
    #[default]
    None,
    Unknown
}

#[cfg(any(feature = "full-stack", feature = "database"))]
#[derive(Debug, Serialize, Deserialize, Clone, Default, PartialEq, sqlx::Type)]
// #[sqlx(type_name = "node_status", rename_all = "snake_case")]
#[sqlx(type_name = "text")]
#[serde(rename_all = "snake_case", tag = "kind", content = "data")]
pub enum K8sType {
    Node,
    Pod,
    #[default]
    None,
    Unknown
}

#[cfg(all(not(feature = "full-stack"), not(feature = "database")))]
#[derive(Debug, Deserialize, Serialize, Clone, PartialEq, Default)]
pub enum NodeType {
    #[default]
    Unknown,
    Custom,
    // CustomNode,
    // CustomPod,
    CustomWithString(String),
    // CustomPodWithString(String),
    // CustomNodeWithString(String),
    // InbuiltNodeWithString(String),
    // InbuiltPodWithString(String),
    InbuiltWithString(String),
    // InbuiltNode,
    // InbuiltPod,
    Inbuilt,
    Main
}

#[cfg(any(feature = "full-stack", feature = "database"))]
// #[derive(Debug, Default, Serialize, Deserialize, Clone, PartialEq, sqlx::Type)]
#[derive(Debug, Default, Serialize, Deserialize, Clone, PartialEq)]
// #[sqlx(type_name = "node_status", rename_all = "snake_case")]
#[serde(rename_all = "snake_case", tag = "kind", content = "data")]
pub enum NodeType {
    #[default]
    Unknown,
    Custom,
    // CustomNode,
    // CustomPod,
    CustomWithString(String),
    // CustomPodWithString(String),
    // CustomNodeWithString(String),
    // InbuiltNodeWithString(String),
    // InbuiltPodWithString(String),
    InbuiltWithString(String),
    // InbuiltNode,
    // InbuiltPod,
    Inbuilt,
    Main
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
#[derive(Debug, Serialize, Deserialize, Clone, Default, PartialEq, sqlx::Type)]
// #[sqlx(type_name = "node_status", rename_all = "snake_case")]
#[sqlx(type_name = "text")]
#[serde(rename_all = "lowercase", tag = "kind", content = "data")]
pub enum Intergrations {
    Minecraft,
    Other,
    #[default]
    Unknown
}

// Ideally I dont hardcode any intergrations like minecraft or any specific provider, but it would be meaningless to move it to its own file when
// its much more readable in this spec, and until i have a better solution down the line or decide to keep this
#[cfg(all(not(feature = "full-stack"), not(feature = "database")))]
#[serde(rename_all = "lowercase")]
#[derive(Debug, Deserialize, Serialize, Clone, PartialEq, Default)]
pub enum Intergrations {
    Minecraft,
    Other,
    #[default]
    Unknown
}

#[cfg(any(feature = "full-stack", feature = "docker", feature = "database"))]
impl Type<Postgres> for NodeType {
    fn type_info() -> sqlx::postgres::PgTypeInfo {
        <String as Type<Postgres>>::type_info()
    }
}


// TODO: Consider removing the string to enum varient matching
impl FromStr for Intergrations {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s {
            "minecraft" => Intergrations::Minecraft,
            "unknown" => Intergrations::Unknown,
            _ => Intergrations::Unknown,
        })
    }
}
impl ToString for Intergrations {

    fn to_string(&self) -> String {
        match self {
            Intergrations::Minecraft => "minecraft".to_string(),
            Intergrations::Unknown => "unknown".to_string(),
            _ => "unknown".to_string(),
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
#[derive(Clone, Debug, sqlx::FromRow, Serialize, Deserialize, PartialEq, Default)]
pub struct Node {
    pub nodename: String,
    pub ip: String,
    pub nodestatus: NodeStatus,
    pub nodetype: NodeType,
    pub k8s_type: K8sType
}


#[cfg(all(not(feature = "full-stack"), not(feature = "database")))]
#[derive(Clone, Debug, Serialize, Deserialize, Default, PartialEq)]
pub struct Node {
    pub nodename: String,
    pub ip: String,
    pub nodestatus: NodeStatus,
    pub nodetype: NodeType,
    pub k8s_type: K8sType
}

#[cfg(any(feature = "full-stack", feature = "database"))]
#[derive(Clone, Debug, sqlx::FromRow, Serialize, Deserialize, PartialEq)]
pub struct Button {
    pub name: String,
    pub link: String,
    pub r#type: String
    //CustomType
}

#[cfg(any(feature = "full-stack", feature = "database"))]
#[derive(Clone, Debug, sqlx::FromRow, Serialize, Deserialize, PartialEq)]
pub struct Intergration {
   // name: String,
    pub status: String,
    pub r#type: Intergrations,
    pub settings: Value
}

#[cfg(all(not(feature = "full-stack"), not(feature = "database")))]
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Intergration {
   // name: String,
    pub status: String,
    pub r#type: Intergrations,
    pub settings: Value
}


#[cfg(all(not(feature = "full-stack"), not(feature = "database")))]
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Button {
    pub name: String,
    pub link: String,
    pub r#type: String
}


#[cfg(any(feature = "full-stack", feature = "database"))]
#[derive(Clone, Debug, sqlx::FromRow, Serialize, Deserialize, Default, PartialEq)]
pub struct Server {
    pub servername: String,
    pub provider: String,
    pub providertype: String,
    pub location: String
}

// I made the mistake of NOT documenting my original plans for provider and providertype, 
// I'll assume provide would have been something like the game, I have no idea for provider type but 
// ill make it represent things within the game, like some game server types maintained by the community
// some using diffrent languages, etc
#[cfg(all(not(feature = "full-stack"), not(feature = "database")))]
#[derive(Clone, Debug, Serialize, Deserialize, Default, PartialEq)]
pub struct Server {
    pub servername: String,
    pub provider: String,
    pub providertype: String,
    pub location: String,
    pub node: Node
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
    async fn get_from_servers_database(&self, servername: &str) -> Result<Option<Server>, Box<dyn Error + Send + Sync>>;
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

pub trait SettingsDatabase {
    async fn set_settings(&self, value: Settings) ->  Result<(), Box<dyn Error + Send + Sync>>;
    async fn get_settings(&self) ->  Result<Settings, Box<dyn Error + Send + Sync>>;
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

pub trait IntergrationsDatabase {
    async fn retrieve_intergrations(&self, intergration: String) -> Option<Intergration>;
    async fn fetch_all_intergrations(&self) -> Result<Vec<Intergration>, Box<dyn Error + Send + Sync>>;
    async fn get_from_intergrations_database(&self, intergration: &str) -> Result<Option<Intergration>, Box<dyn Error + Send + Sync>>;
    async fn create_intergrations_in_db(&self, intergration: ModifyElementData) -> Result<StatusCode, Box<dyn Error + Send + Sync>>;
    async fn remove_intergrations_in_db(&self, intergration: ModifyElementData) -> Result<StatusCode, Box<dyn Error + Send + Sync>>;
    async fn edit_intergrations_in_db(&self, intergration: ModifyElementData) -> Result<StatusCode, Box<dyn Error + Send + Sync>>;
}
