use crate::Deserialize;
use crate::Serialize;
use crate::StatusCode;
use std::error::Error;
use std::fmt;

use serde_json::Value;

#[cfg(any(feature = "full-stack", feature = "database"))]
use sqlx::{
    Decode, Encode, Postgres, Type,
    postgres::{PgArgumentBuffer, PgValueRef},
};

use std::str::FromStr;

#[allow(unused)]
use serde::ser::StdError;

#[allow(unused)]
type BoxDynError = Box<dyn StdError + Send + Sync>;

// #[derive(Debug)]
// pub struct DatabaseError(pub StatusCode);

#[derive(Debug, Clone)]
pub enum DatabaseError {
    StatusCode(StatusCode),
}

impl fmt::Display for DatabaseError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            DatabaseError::StatusCode(status_code) => write!(f, "HTTP error: {}", status_code),
        }
    }
}

impl Error for DatabaseError {}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct RetrieveElement {
    pub element: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(tag = "kind", content = "data")]
pub enum Element {
    User {
        password: String,
        user: String,
        user_perms: Vec<UserPerm>,
    },
    Node(Node),
    Button(Button),
    Server(Server),
    Intergration(Intergration),
    String(String),
}
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct ModifyElementData {
    pub element: Element,
    pub jwt: String,
    pub require_auth: bool,
}

#[cfg_attr(
    any(feature = "full-stack", feature = "database"),
    derive(sqlx::Type)
)]
#[cfg_attr(
    any(feature = "full-stack", feature = "database"),
    sqlx(type_name = "TEXT", rename_all = "PascalCase")
)]
#[derive(Debug, Deserialize, Serialize, Clone, PartialEq)]
#[serde(rename_all = "snake_case", tag = "kind", content = "data")]
pub enum Filters {
    AlternatingLine,
    None,
}


#[cfg_attr(
    any(feature = "full-stack", feature = "database"),
    derive(sqlx::Type)
)]
#[cfg_attr(
    any(feature = "full-stack", feature = "database"),
    sqlx(type_name = "TEXT", rename_all = "PascalCase")
)]
#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "snake_case", tag = "kind", content = "data")]
pub enum FileSystemDrivers {
    Tcp,
    None,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[cfg_attr(
    any(feature = "full-stack", feature = "database"),
    derive(sqlx::FromRow)
)]
pub struct Settings {
    pub(crate) toggled_default_buttons: bool,
    pub(crate) status_type: String,
    pub(crate) enabled_rcon: bool,
    pub(crate) rcon_url: String,
    pub(crate) rcon_password: String,
    //pub(crate) driver: String,
    pub(crate) filter: Filters,
    pub(crate) file_system_driver: FileSystemDrivers,
    pub(crate) enable_statistics_on_home_page: bool,
    pub(crate) enable_nodes_on_home_page: bool,
    pub(crate) console_entry_on_top: bool,
    pub(crate) force_sandbox: bool,
    pub(crate) disable_custom_servers: bool,
    #[cfg_attr(any(feature = "full-stack", feature = "database"), sqlx(json))]
    pub(crate) current_server: Server,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            toggled_default_buttons: Default::default(),
            status_type: Default::default(),
            enabled_rcon: true,
            rcon_url: "localhost:25575".to_string(),
            rcon_password: "testing".to_string(),
            filter: Filters::None,
            //driver: "".to_string(),
            enable_statistics_on_home_page: false,
            enable_nodes_on_home_page: false,
            console_entry_on_top: true,
            force_sandbox: false,
            disable_custom_servers: false,
            file_system_driver: FileSystemDrivers::None,
            current_server: Server::default().into(),
        }
    }
}


#[derive(Debug, Serialize, Clone, PartialEq, Default)]
#[cfg_attr(any(feature = "full-stack", feature = "database"), derive(sqlx::Type))]
#[cfg_attr(
    any(feature = "full-stack", feature = "database"),
    sqlx(type_name = "text")
)]
#[serde(rename_all = "snake_case", tag = "kind", content = "data")]
pub enum NodeStatus {
    #[default]
    Unknown,
    Enabled,
    Disabled,
    ImmutablyEnabled,
    ImmutablyDisabled,
}

impl<'de> serde::Deserialize<'de> for NodeStatus {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let v = serde_json::Value::deserialize(d)?;
        let s = match &v {
            serde_json::Value::String(s) => s.clone(),
            serde_json::Value::Object(map) => map
                .get("kind")
                .and_then(|k| k.as_str())
                .unwrap_or("unknown")
                .to_string(),
            _ => "unknown".to_string(),
        };
        Ok(match s.to_lowercase().as_str() {
            "enabled" => NodeStatus::Enabled,
            "disabled" => NodeStatus::Disabled,
            "immutablyenabled" | "immutably_enabled" => NodeStatus::ImmutablyEnabled,
            "immutablydisabled" | "immutably_disabled" => NodeStatus::ImmutablyDisabled,
            _ => NodeStatus::Unknown,
        })
    }
}

impl<'de> serde::Deserialize<'de> for K8sType {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let s = String::deserialize(d)?;
        Ok(match s.to_lowercase().as_str() {
            "node" => K8sType::Node,
            "pod" => K8sType::Pod,
            "inbuilt" => K8sType::Inbuilt,
            "unknown" => K8sType::Unknown,
            _ => K8sType::None,
        })
    }
}

#[derive(Debug, Serialize, Clone, Default, PartialEq)]
#[cfg_attr(any(feature = "full-stack", feature = "database"), derive(sqlx::Type))]
#[cfg_attr(
    any(feature = "full-stack", feature = "database"),
    sqlx(type_name = "text")
)]
//#[serde(rename_all = "snake_case", tag = "kind", content = "data")]
#[serde(rename_all = "lowercase")]
pub enum K8sType {
    Node,
    Pod,
    #[default]
    None,
    Inbuilt,
    Unknown,
}


impl TryFrom<String> for K8sType {
    type Error = Box<dyn Error + Send + Sync>;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        match value.as_str() {
            "None" => Ok(K8sType::None),
            "Inbuilt" => Ok(K8sType::Inbuilt),
            "Pod" => Ok(K8sType::Pod),
            "Node" => Ok(K8sType::Node),
            "Unknown" => Ok(K8sType::Unknown),
            other => Err(format!("unknown K8sType: {other}").into()),
        }
    }
}

pub struct K8sNode {
    pub name: String,
    pub ip: String,
    pub gameserver: String,
    pub k8s_type: K8sType,
}

impl<'de> serde::Deserialize<'de> for NodeType {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let v = serde_json::Value::deserialize(d)?;
        match &v {
            serde_json::Value::String(s) => Ok(NodeType::from(s.to_lowercase())),
            serde_json::Value::Object(map) => {
                if let Some(serde_json::Value::String(kind)) = map.get("kind") {
                    Ok(NodeType::from(kind.to_lowercase()))
                } else {
                    Ok(NodeType::Unknown)
                }
            }
            _ => Ok(NodeType::Unknown),
        }
    }
}

#[derive(Debug, Default, Serialize, Clone, PartialEq)]
#[serde(rename_all = "snake_case", tag = "kind", content = "data")]
pub enum NodeType {
    #[default]
    Unknown,
    Custom(Option<String>),
    // CustomNode,
    // CustomPod,
    // CustomWithString(String),
    // CustomPodWithString(String),
    // CustomNodeWithString(String),
    // InbuiltNodeWithString(String),
    // InbuiltPodWithString(String),
    // #[allow(unused)]
    // InbuiltWithString(String),
    // InbuiltNode,
    // InbuiltPod,
    Inbuilt,
    Main,
}
#[cfg(any(feature = "full-stack", feature = "database"))]
impl<'r> Decode<'r, Postgres> for NodeType {
    fn decode(value: PgValueRef<'r>) -> Result<Self, BoxDynError> {
        let s = <String as Decode<Postgres>>::decode(value)?;
        Ok(NodeType::from(s))
    }
}

#[cfg(any(feature = "full-stack", feature = "database"))]
impl<'q> Encode<'q, Postgres> for NodeType {
    fn encode_by_ref(
        &self,
        buf: &mut PgArgumentBuffer,
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
            "custom" => NodeType::Custom(None),
            "inbuilt" => NodeType::Inbuilt,
            "main" => NodeType::Main,
            other => NodeType::Custom(Some(other.to_string())),
        }
    }
}

impl ToString for NodeType {
    fn to_string(&self) -> String {
        match self {
            NodeType::Custom(None) => "custom".to_string(),
            NodeType::Custom(Some(custom)) => custom.to_string(),
            NodeType::Inbuilt => "inbuilt".to_string(),
            NodeType::Main => "main".to_string(),
            _ => String::new(),
        }
    }
}

// Ideally I dont hardcode any intergrations like minecraft or any specific provider, but it would be meaningless to move it to its own file when
// its much more readable in this spec, and until i have a better solution down the line or decide to keep this
// #[cfg(any(feature = "full-stack", feature = "database"))]
#[derive(Debug, Serialize, Deserialize, Clone, Default, PartialEq)]
// #[sqlx(type_name = "node_status", rename_all = "snake_case")]
#[cfg_attr(any(feature = "full-stack", feature = "database"), derive(sqlx::Type))]
#[cfg_attr(
    any(feature = "full-stack", feature = "database"),
    sqlx(type_name = "text")
)]
#[serde(rename_all = "lowercase", tag = "kind", content = "data")]
pub enum Intergrations {
    Minecraft,
    Other,
    #[default]
    Unknown,
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

// TODO: consider if in the future instead of encoding json into a string i use jsonb instead
#[derive(Deserialize, Serialize, Clone, Debug, PartialEq)]
#[cfg_attr(
    any(feature = "full-stack", feature = "database"),
    derive(sqlx::FromRow)
)]
pub struct UserPerm {
    pub(crate) perm: String,
    pub(crate) scope: String
}

impl fmt::Display for UserPerm {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.perm, self.scope)
    }
}


#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct User {
    pub username: String,
    pub password_hash: Option<String>,
    pub user_perms: Vec<UserPerm>,
}

#[cfg(any(feature = "full-stack", feature = "database"))]
impl<'r> sqlx::FromRow<'r, sqlx::postgres::PgRow> for User {
    fn from_row(row: &'r sqlx::postgres::PgRow) -> Result<Self, sqlx::Error> {
        use sqlx::Row;

        let username: String = row.try_get("username")?;
        let password_hash: Option<String> = row.try_get("password_hash")?;
        let raw_perms: Vec<String> = row.try_get("user_perms")?;

        let user_perms = raw_perms
            .iter()
            .filter_map(|s| {
                let (perm, scope) = s.split_once(':')?;
                Some(UserPerm {
                    perm: perm.to_string(),
                    scope: scope.to_string(),
                })
            })
            .collect();

        Ok(User { username, password_hash, user_perms })
    }
}


#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Default)]
#[cfg_attr(
    any(feature = "full-stack", feature = "database"),
    derive(sqlx::FromRow, Decode, Encode)
)]
pub struct Node {
    pub nodename: String,
    pub ip: String,
    pub nodestatus: NodeStatus,
    pub nodetype: NodeType,
    //#[sqlx(rename = "nodetype")]
    #[cfg_attr(any(feature = "full-stack", feature = "database"), sqlx(skip))]
    pub k8s_type: K8sType,
}

#[cfg(any(feature = "full-stack", feature = "docker", feature = "database"))]
impl Type<Postgres> for Node {
    fn type_info() -> sqlx::postgres::PgTypeInfo {
        <String as Type<Postgres>>::type_info()
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[cfg_attr(
    any(feature = "full-stack", feature = "database"),
    derive(sqlx::FromRow)
)]
pub struct Button {
    pub name: String,
    pub link: String,
    pub r#type: String, //CustomType
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[cfg_attr(
    any(feature = "full-stack", feature = "database"),
    derive(sqlx::FromRow)
)]
pub struct Intergration {
    // name: String,
    pub status: String,
    pub r#type: Intergrations,
    pub settings: Value,
}


#[derive(Clone, Debug, Serialize, Deserialize, Default, PartialEq)]
#[cfg_attr(
    any(feature = "full-stack", feature = "database"),
    derive(sqlx::Type)
)]
pub struct ServerMetadata {
    start_keyword: Option<String>,
    stop_keyword: Option<String>
}

// I made the mistake of NOT documenting my original plans for provider and providertype,
// I'll assume provide would have been something like the game, I have no idea for provider type but
// ill make it represent things within the game, like some game server types maintained by the community
// some using diffrent languages, etc
#[derive(Clone, Debug, Serialize, Deserialize, Default, PartialEq)]
#[cfg_attr(
    any(feature = "full-stack", feature = "database"),
    derive(sqlx::FromRow, Decode, Encode)
)]
pub struct Server {
    #[serde(default)]
    pub servername: String,
    #[serde(default)]
    pub provider: String,
    #[serde(default)]
    pub providertype: String,
    #[serde(default)]
    pub location: String,
    #[cfg_attr(any(feature = "full-stack", feature = "database"), sqlx(json))]
    #[serde(default)]
    pub node: Node,
    #[serde(default)]
    pub sandbox: bool,
    #[cfg_attr(any(feature = "full-stack", feature = "database"), sqlx(json))]
    #[serde(default)]
    pub server_metadata: ServerMetadata,
}

#[cfg(any(feature = "full-stack", feature = "docker", feature = "database"))]
impl Type<Postgres> for Server {
    fn type_info() -> sqlx::postgres::PgTypeInfo {
        <String as Type<Postgres>>::type_info()
    }
}

pub trait IntoServer {
    fn into_server(self) -> Server;
}

impl IntoServer for Server {
    fn into_server(self) -> Server {
        self
    }
}

#[cfg(any(feature = "full-stack", feature = "database"))]
impl IntoServer for sqlx::types::Json<Server> {
    fn into_server(self) -> Server {
        self.0
    }
}

// #[async_trait]
pub trait UserDatabase {
    async fn retrieve_user(&self, username: String) -> Option<User>;
    async fn fetch_all(&self) -> Result<Vec<User>, Box<dyn Error + Send + Sync>>;
    async fn get_user_from_database(
        &self,
        username: &str,
    ) -> Result<Option<User>, Box<dyn Error + Send + Sync>>;
    async fn create_user_in_db(
        &self,
        user: ModifyElementData,
    ) -> Result<StatusCode, Box<dyn Error + Send + Sync>>;
    async fn remove_user_in_db(
        &self,
        user: ModifyElementData,
    ) -> Result<StatusCode, Box<dyn Error + Send + Sync>>;
    async fn edit_user_in_db(
        &self,
        user: ModifyElementData,
    ) -> Result<StatusCode, Box<dyn Error + Send + Sync>>;
}

pub trait ServerDatabase {
    async fn retrieve_server(&self, servername: String) -> Option<Server>;
    async fn fetch_all_servers(&self) -> Result<Vec<Server>, Box<dyn Error + Send + Sync>>;
    async fn get_from_servers_database(
        &self,
        servername: &str,
    ) -> Result<Option<Server>, Box<dyn Error + Send + Sync>>;
    async fn create_server_in_db(
        &self,
        server: ModifyElementData,
    ) -> Result<StatusCode, Box<dyn Error + Send + Sync>>;
    async fn remove_server_in_db(
        &self,
        server: ModifyElementData,
    ) -> Result<StatusCode, Box<dyn Error + Send + Sync>>;
    async fn simple_remove_server_in_db(
        &self, 
        servername: String
    ) -> Result<StatusCode, Box<dyn Error + Send + Sync>>;
    async fn edit_server_in_db(
        &self,
        server: ModifyElementData,
    ) -> Result<StatusCode, Box<dyn Error + Send + Sync>>;
}
pub trait NodesDatabase {
    async fn retrieve_nodes(&self, nodename: String) -> Option<Node>;
    async fn fetch_all_nodes(&self) -> Result<Vec<Node>, Box<dyn Error + Send + Sync>>;
    async fn get_from_nodes_database(
        &self,
        nodename: &str,
    ) -> Result<Option<Node>, Box<dyn Error + Send + Sync>>;
    async fn create_nodes_in_db(
        &self,
        node: ModifyElementData,
    ) -> Result<StatusCode, Box<dyn Error + Send + Sync>>;
    async fn remove_node_in_db(
        &self,
        node: ModifyElementData,
    ) -> Result<StatusCode, Box<dyn Error + Send + Sync>>;
    async fn remove_node_in_db_directly(
        &self,
        node: Node,
    ) -> Result<StatusCode, Box<dyn Error + Send + Sync>>;
    async fn edit_node_in_db(
        &self,
        node: ModifyElementData,
    ) -> Result<StatusCode, Box<dyn Error + Send + Sync>>;
}

pub fn resolve_database_error_into_statuscode(error: DatabaseError) -> StatusCode {
    match error {
        DatabaseError::StatusCode(status_code) => status_code,
    }
}

pub trait SettingsDatabase {
    async fn set_settings(&self, value: Settings) -> Result<(), Box<dyn Error + Send + Sync>>;
    async fn get_settings(&self) -> Result<Settings, Box<dyn Error + Send + Sync>>;
}
pub trait ButtonsDatabase {
    async fn retrieve_buttons(&self, name: String) -> Option<Button>;
    async fn fetch_all_buttons(&self) -> Result<Vec<Button>, Box<dyn Error + Send + Sync>>;
    async fn toggle_default_buttons(&self) -> Result<StatusCode, Box<dyn Error + Send + Sync>>;
    async fn toggle_button_state(&self) -> Result<bool, Box<dyn Error + Send + Sync>>;
    async fn reset_buttons(&self) -> Result<StatusCode, Box<dyn Error + Send + Sync>>;
    async fn get_from_buttons_database(
        &self,
        name: &str,
    ) -> Result<Option<Button>, Box<dyn Error + Send + Sync>>;
    async fn edit_button_in_db(
        &self,
        button: ModifyElementData,
    ) -> Result<StatusCode, Box<dyn Error + Send + Sync>>;
}

pub trait IntergrationsDatabase {
    async fn retrieve_intergrations(&self, intergration: String) -> Option<Intergration>;
    async fn fetch_all_intergrations(
        &self,
    ) -> Result<Vec<Intergration>, Box<dyn Error + Send + Sync>>;
    async fn get_from_intergrations_database(
        &self,
        intergration: &str,
    ) -> Result<Option<Intergration>, Box<dyn Error + Send + Sync>>;
    async fn create_intergrations_in_db(
        &self,
        intergration: ModifyElementData,
    ) -> Result<StatusCode, Box<dyn Error + Send + Sync>>;
    async fn remove_intergrations_in_db(
        &self,
        intergration: ModifyElementData,
    ) -> Result<StatusCode, Box<dyn Error + Send + Sync>>;
    async fn edit_intergrations_in_db(
        &self,
        intergration: ModifyElementData,
    ) -> Result<StatusCode, Box<dyn Error + Send + Sync>>;
}
