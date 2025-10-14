use mime_guess::mime::Name;
use serde::de::value;
use serde::Deserialize;
use serde::Serialize;
use std::error::Error;
use std::result;
pub mod databasespec;
pub use databasespec::{User, Node, Element, ModifyElementData, UserDatabase, NodesDatabase, RetrieveElement, DatabaseError};
// use std::path::Path;
use std::path::PathBuf;
use std::fs::OpenOptions;
use std::fs::File;
use std::io::Write;
use std::io::Read;
use std::collections::HashMap;

use crate::database::databasespec::Server;
use crate::database::databasespec::ServerDatabase;
use crate::database::databasespec::Settings;
use crate::database::databasespec::NodeType;
// use crate::database::databasespec::CustomType;
use crate::database::databasespec::Button;
use crate::database::databasespec::ButtonsDatabase;

use crate::database::databasespec::SettingsDatabase;
// use crate::database::Database;
use crate::StatusCode;

#[derive(Clone)]
pub struct JsonBackend {
    file: PathBuf
}

#[derive(Deserialize, Serialize, Debug)]
pub struct JsonBackendContent {
    pub users: Vec<User>,
    pub nodes: Vec<Node>,
    pub servers: Vec<Server>,
    pub buttons: Vec<Button>,
    pub toggled_buttons: Vec<Button>,
    pub settings: Settings
    //pub buttons: HashMap<String, Node>
}
impl Default for JsonBackendContent {
    fn default() -> JsonBackendContent {
        let default_button_list = vec![
            Button {
                name: "Filebrowser".to_string(),
                link: "".to_string(),
                r#type: "default".to_string()
                //CustomType::Default
            },
            Button {
                name: "Statistics".to_string(),
                link: "".to_string(),
                r#type: "default".to_string()
            },
            // Button {
            //     name: "Scedules".to_string(),
            //     link: "".to_string(),
            //     r#type: "default".to_string()
            //     //CustomType::Default
            // },
            Button {
                name: "Workflows".to_string(),
                link: "".to_string(),
                r#type: "default".to_string()
                //CustomType::Default
            },
            Button {
                name: "Intergrations".to_string(),
                link: "".to_string(),
                r#type: "default".to_string()
                //CustomType::Default
            },
            Button {
                name: "Backups".to_string(),
                link: "".to_string(),
                r#type: "default".to_string()
            },
            Button {
                name: "Settings".to_string(),
                link: "".to_string(),
                r#type:  "default".to_string()
            }
        ];
        JsonBackendContent {
            users: vec![],
            nodes: vec![],
            servers: vec![],
            buttons: default_button_list.clone(),
            toggled_buttons: default_button_list,
            settings: Settings::default()
        }
    }
}

#[derive(Clone)]
pub struct Database {
    pub connection: JsonBackend
}
impl Default for JsonBackend {
    fn default() -> Self {
        JsonBackend {
            file: PathBuf::from("credentials.json")
        }
    }
}
impl JsonBackend {
    pub fn new(mut file: Option<PathBuf>) -> Self {
        if let Some(path) = &file {
            let _ = OpenOptions::new()
                .create(true)
                .write(true)
                .open(path);
        } else {
            let _ = OpenOptions::new()
                .create(true)
                .write(true)
                .open(JsonBackend::default().file);
            file = Some(JsonBackend::default().file);
        }
        let mut open_file = OpenOptions::new()
            .write(true)
            .read(true)
            .open(&file.clone().unwrap())
            .map_err(|e| format!("Failed to open file: {}", e)).unwrap();

        let mut rewrite_file: bool = false;
        let mut database: JsonBackendContent = 
            serde_json::from_reader(&open_file).unwrap_or_else(|e| {
                let mut backup_path = file.clone().unwrap();
                backup_path.set_extension("old");
                rewrite_file = true;
                println!("Failed to parse JSON: {}", e); 
                JsonBackendContent::default()
            });
        if rewrite_file {
            open_file.write_all(serde_json::to_string_pretty(&database).unwrap().as_bytes());
        }
        if file.is_some() {
            JsonBackend {
                file: file.unwrap()
            }
        } else {
            JsonBackend::default()
        }
    }
}

impl Database {
    pub fn new(conn: Option<JsonBackend>) -> Database {  
        let connection = conn.unwrap_or_default();
        Database {
            connection,
        }
    }
    pub async fn clear_db(&self) -> Result<(), String> {
        let clear_file = OpenOptions::new()
            .write(true)
            .truncate(true)
            .read(true)
            .open(&self.connection.file)
            .map_err(|e| format!("Failed to open file: {}", e));
        if clear_file.is_err(){
            println!("{:#?}", clear_file);
            return Err("Error".to_string())
        }

        Ok(())
    }
    async fn write_database(&self, database: JsonBackendContent) -> Result<String, String> {
        let file_path = &self.connection.file;
        let json = serde_json::to_string_pretty(&database)
            .map_err(|e| format!("Failed to serialize JSON: {}", e))?;

        let mut file = OpenOptions::new()
            .write(true)
            .truncate(true)
            .open(file_path)
            .map_err(|e| format!("Failed to open file for writing: {}", e))?;

        file.write_all(json.as_bytes())
            .map_err(|e| format!("Write error: {}", e))?;

        file.sync_all()
            .map_err(|e| format!("Failed to sync data to disk: {}", e))?;

        Ok("Wrote file successfully".to_string())
    }
    async fn get_database(&self) -> Result<JsonBackendContent, String> {
        let file_path = &self.connection.file;
    
        let mut read_file = File::open(file_path)
            .map_err(|e| format!("Failed to open file: {}", e))?;
        let mut contents = String::new();
        read_file.read_to_string(&mut contents).map_err(|e| format!("Read error: {}", e))?;
    
        let mut database: Result<JsonBackendContent, String> = if contents.trim().is_empty() {
            Ok(JsonBackendContent::default())
        } else {
            //println!("{:#?}", contents.clone());
            let result: Result<JsonBackendContent, String> = serde_json::from_str(&contents).map_err(|e| {
                println!("Failed to parse JSON (1): {}", e); 
                format!("Error: {}", e)
            });
            if result.is_err(){
                Err(result.err().unwrap())
            } else {
                Ok(result?)
            }
        };
        database
    }
}

impl ServerDatabase for Database {
    async fn retrieve_server(&self, servername: String) -> Option<Server> {
         let database = self.get_database().await;
        database.unwrap().servers.iter().find(|server| server.servername == servername).cloned()
    }
    async fn fetch_all_servers(&self) -> Result<Vec<Server>, Box<dyn Error + Send + Sync>> {
        let database = self.get_database().await?;
        Ok(database.servers)
    }
    async fn get_from_servers_database(&self, servername: &str) -> Result<Option<Server>, Box<dyn Error + Send + Sync>> {
        let database = self.get_database().await?;
        Ok(database.servers.iter().find(|server| server.servername == servername).cloned())  
    }
    async fn create_server_in_db(&self, element: ModifyElementData) -> Result<StatusCode, Box<dyn Error + Send + Sync>> {
        if let Element::Server(Server {servername, provider, providertype, location }) = element.element {
            let mut database = self.get_database().await?;
    
            if database.servers.iter().any(|server| server.servername == servername){
                return Err(Box::new(DatabaseError(StatusCode::INTERNAL_SERVER_ERROR)));
            } else {
                let server = Server { servername, provider, providertype, location };
                database.servers.push(server.clone());
            }
        
            self.write_database(database).await;  
            Ok(StatusCode::CREATED)
        } else {
            Err(Box::new(DatabaseError(StatusCode::INTERNAL_SERVER_ERROR)))
        }
    }
    async fn remove_server_in_db(&self, element: ModifyElementData) -> Result<StatusCode, Box<dyn Error + Send + Sync>> {
        let mut database = self.get_database().await?;
        if let Element::Server(Server {servername, provider, providertype, location }) = element.element {
            database.servers.retain(|db_server| db_server.servername != servername);
        } else {
            return Err(Box::new(DatabaseError(StatusCode::INTERNAL_SERVER_ERROR)));
        }

        self.write_database(database).await;

        Ok(StatusCode::CREATED)
    }
    async fn edit_server_in_db(&self, element: ModifyElementData) -> Result<StatusCode, Box<dyn Error + Send + Sync>> {
        if let Element::Server(Server {servername, provider, providertype, location }) = element.element {
            let mut database = self.get_database().await?;
            if let Some(db_server) = database.servers.iter_mut().find(|db_server| db_server.servername == servername) {
                // db_server.user_perms = user_perms.clone();
                // db_server.username = user.clone();
            }

            self.write_database(database).await;
            Ok(StatusCode::CREATED)
        } else {
            Err(Box::new(DatabaseError(StatusCode::INTERNAL_SERVER_ERROR)))
        }
    }
}
// pub struct Server {
//     pub servername: String,
//     pub provider: String,
//     pub providertype: String
// } 

impl UserDatabase for Database {
    async fn retrieve_user(&self, username: String) -> Option<User> {
        let enable_admin_user = std::env::var("ENABLE_ADMIN_USER").unwrap_or_default() == "true";
        let admin_user = std::env::var("ADMIN_USER").unwrap_or_default();
        let admin_password = std::env::var("ADMIN_PASSWORD").unwrap_or_default();
        if username == admin_user && enable_admin_user {
            let password_hash = bcrypt::hash(admin_password, bcrypt::DEFAULT_COST).ok();
            return Some(User{
                username,
                password_hash, 
                user_perms: vec!["all".to_string()]
            });
        } if let Ok(Some(user)) = self.get_from_database(&username.clone()).await {
            Some(user)
        } else {
            None
        }

    }
    async fn fetch_all(&self) -> Result<Vec<User>, Box<dyn Error + Send + Sync>>{
        let database = self.get_database().await?;

        
        Ok(database.users)
    }
    async fn get_from_database(&self, username: &str) -> Result<Option<User>, Box<dyn Error + Send + Sync>> { 
        let database = self.get_database().await?;
        if let Some(user) = database.users.iter().find(|user| user.username == username){
            Ok(Some(user.clone()))
        } else {
            Ok(None)
        }
    }
  
    
    async fn create_user_in_db(&self, element: ModifyElementData) -> Result<StatusCode, Box<dyn Error + Send + Sync>> {
        if let Element::User { password, user, user_perms } = element.element {
            let password_hash = bcrypt::hash(password.clone(), bcrypt::DEFAULT_COST);
            let mut database = self.get_database().await?;
    
            if password.is_empty() || database.users.iter().any(|db_user| db_user.username == user){
                return Err(Box::new(DatabaseError(StatusCode::INTERNAL_SERVER_ERROR)));
            } else {
                let final_user = User {
                    username: user,
                    password_hash: Some(password_hash.unwrap()),
                    user_perms: user_perms
                };
                database.users.push(final_user.clone());
            }
        
            self.write_database(database).await;  
            Ok(StatusCode::CREATED)
        } else {
            Err(Box::new(DatabaseError(StatusCode::INTERNAL_SERVER_ERROR)))
        }
    }
    
    async fn remove_user_in_db(&self, user: ModifyElementData) -> Result<StatusCode, Box<dyn Error + Send + Sync>> {
        let file_path = &self.connection.file;
    
        let mut read_file = File::open(file_path)
            .map_err(|e| format!("Failed to open file: {}", e))?;
        let mut contents = String::new();
        read_file.read_to_string(&mut contents).map_err(|e| format!("Read error: {}", e))?;
    
        let mut database: JsonBackendContent = if contents.trim().is_empty() {
            JsonBackendContent::default()
        } else {
            serde_json::from_str(&contents).map_err(|e| {
                println!("Failed to parse JSON: {}", e); 
                format!("Error: {}", e)
            })?
        };
        if let Element::User { user, user_perms: _, password: _ } = user.element {
            database.users.retain(|db_user| db_user.username != user);
        } else {
            return Err(Box::new(DatabaseError(StatusCode::INTERNAL_SERVER_ERROR)));
        }

        self.write_database(database).await;
        Ok(StatusCode::CREATED)
    }
    async fn edit_user_in_db(&self, element: ModifyElementData) -> Result<StatusCode, Box<dyn Error + Send + Sync>> {
        if let Element::User { password, user, user_perms } = element.element {
            let mut database = self.get_database().await?;
            if let Some(db_user) = database.users.iter_mut().find(|db_user| db_user.username == user) {
                if !password.is_empty() {
                    db_user.password_hash = bcrypt::hash(password.clone(), bcrypt::DEFAULT_COST).ok();
                }
                db_user.user_perms = user_perms.clone();
                db_user.username = user.clone();
            }

            self.write_database(database).await;
            Ok(StatusCode::CREATED)
        } else {
            Err(Box::new(DatabaseError(StatusCode::INTERNAL_SERVER_ERROR)))
        }
    }
}

impl NodesDatabase for Database {
    async fn retrieve_nodes(&self, nodename: String) -> Option<Node> {
        let database = self.get_database().await;
        database.unwrap().nodes.iter().find(|node| node.nodename == nodename).cloned()
    }
    async fn fetch_all_nodes(&self) -> Result<Vec<Node>, Box<dyn Error + Send + Sync>> {
        let database = self.get_database().await?;
        Ok(database.nodes)
    }
    async fn get_from_nodes_database(&self, nodename: &str) -> Result<Option<Node>, Box<dyn Error + Send + Sync>> {
        let database = self.get_database().await?;
        Ok(database.nodes.iter().find(|node| node.nodename == nodename).cloned())
    }
    async fn create_nodes_in_db(&self, element: ModifyElementData) -> Result<StatusCode, Box<dyn Error + Send + Sync>> {
        if let Element::Node(Node { nodename, ip, nodetype, nodestatus, k8s_type }) = element.element {
            let mut database = self.get_database().await?;
    
            if database.nodes.iter().any(|node| node.nodename == nodename) || (nodetype == NodeType::Custom && ip.is_empty()){
                return Err(Box::new(DatabaseError(StatusCode::INTERNAL_SERVER_ERROR)));
            } else {
                let final_node = Node {nodename,ip,nodetype, nodestatus, k8s_type };
                database.nodes.push(final_node.clone());
            }
        
            self.write_database(database).await;  
            Ok(StatusCode::CREATED)
        } else {
            Err(Box::new(DatabaseError(StatusCode::INTERNAL_SERVER_ERROR)))
        }
    }
    async fn remove_node_in_db(&self, node: ModifyElementData) -> Result<StatusCode, Box<dyn Error + Send + Sync>> {
        todo!()
    }
    async fn edit_node_in_db(&self, node: ModifyElementData) -> Result<StatusCode, Box<dyn Error + Send + Sync>> {
        todo!()
    }
}

impl ButtonsDatabase for Database {
    async fn retrieve_buttons(&self, name: String) -> Option<Button> {
        todo!()
    }
    async fn fetch_all_buttons(&self) -> Result<Vec<Button>, Box<dyn Error + Send + Sync>> {
        let database = self.get_database().await?;
        Ok(database.buttons)
    }
    async fn get_from_buttons_database(&self, name: &str) -> Result<Option<Button>, Box<dyn Error + Send + Sync>> {
        let database = self.get_database().await?;
        //database.buttons.get(name)
        Ok(database.buttons.iter().find(|button| button.name == name).cloned())
    }
    async fn toggle_button_state(&self) -> Result<bool, Box<dyn Error + Send + Sync>> {
        let database = self.get_database().await?;
        Ok(database.settings.toggled_default_buttons)
    }
    async fn toggle_default_buttons(&self) -> Result<StatusCode, Box<dyn Error + Send + Sync>> {
        let mut database: JsonBackendContent = self.get_database().await?;
        if database.settings.toggled_default_buttons == false {
            database.toggled_buttons = JsonBackendContent::default().buttons;
        }
        std::mem::swap(&mut database.buttons, &mut database.toggled_buttons);
        database.settings.toggled_default_buttons = !database.settings.toggled_default_buttons;
        self.write_database(database).await?;
        Ok(StatusCode::CREATED)
    }
    async fn reset_buttons(&self) -> Result<StatusCode, Box<dyn Error + Send + Sync>> {
        let mut database: JsonBackendContent = self.get_database().await?;
        database.buttons = JsonBackendContent::default().buttons;
        database.toggled_buttons = JsonBackendContent::default().buttons;
        self.write_database(database).await?;
        Ok(StatusCode::CREATED)
    }

    async fn edit_button_in_db(&self, element: ModifyElementData) -> Result<StatusCode, Box<dyn Error + Send + Sync>>{
        if let Element::Button(button) = element.element {
            if let Button { name, link, r#type } = button {
                let mut database = self.get_database().await?;
                // println!("{}", name);
                if let Some(db_button) = database.buttons.iter_mut().find(|db_button| db_button.name.to_lowercase()  == name.to_lowercase() ) {
                    // println!("{}", db_button.link);
                    db_button.link = link.clone();
                    db_button.r#type = "custom".to_string(); 
                    //CustomType::Custom; 
                    // println!("{}", db_button.link);
                }
                //println!("Editing button");
                self.write_database(database).await?;
                Ok(StatusCode::CREATED)
            } else {
                println!("Error, failed to get the underlying items");
                Err(Box::new(DatabaseError(StatusCode::INTERNAL_SERVER_ERROR)))
            }
        } else {
            println!("Error, failed to get the button element type");
            Err(Box::new(DatabaseError(StatusCode::INTERNAL_SERVER_ERROR)))
        }
    }
}

impl SettingsDatabase for Database {
    async fn set_settings(&self, settings: Settings) ->  Result<(), Box<dyn Error + Send + Sync>> {
        let mut database = self.get_database().await?;
        database.settings = settings;
        self.write_database(database).await?;
        Ok(())
    }
    async fn get_settings(&self) ->  Result<Settings, Box<dyn Error + Send + Sync>> {
        let database = self.get_database().await?;
        Ok(database.settings)
    }
}