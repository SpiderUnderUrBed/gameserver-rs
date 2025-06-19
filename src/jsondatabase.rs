use serde::Deserialize;
use serde::Serialize;
// use std::path::Path;
use std::path::PathBuf;
use std::fs::OpenOptions;
use std::fs::File;
use std::io::Write;
use std::io::Read;


#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct User {
    pub username: String,
    pub password_hash: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct CreateUserData {
    pub user: String,
    pub password: String,
    pub authcode: String
}
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct RemoveUserData {
    pub user: String,
    pub authcode: String
}

#[derive(Clone)]
pub struct JsonBackend {
    file: PathBuf
}

#[derive(Deserialize, Serialize, Default, Debug)]
pub struct JsonBackendContent {
    pub users: Vec<User>
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
    pub async fn retrive_user(&self, username: String) -> Option<User> {
        let enable_admin_user = std::env::var("ENABLE_ADMIN_USER").unwrap_or_default() == "true";
        let admin_user = std::env::var("ADMIN_USER").unwrap_or_default();
        let admin_password = std::env::var("ADMIN_PASSWORD").unwrap_or_default();
        if username == admin_user && enable_admin_user {
            let password_hash = bcrypt::hash(admin_password, bcrypt::DEFAULT_COST).ok();
            return Some(User{
                username,
                password_hash
            });
        } if let Ok(Some(user)) = self.get_from_database(&username.clone()).await {
            Some(user)
        } else {
            None
        }

    }
    pub async fn fetch_all(&self, item: &str) -> Result<Vec<User>, String>{
        let mut file = OpenOptions::new()
        .write(true)
        .read(true)
        .open(&self.connection.file)
        .map_err(|e| format!("Failed to open file: {}", e))?;

        let database: JsonBackendContent = serde_json::from_reader(&file)
            .map_err(|e| format!("Failed to parse JSON: {}", e))?;

        
        Ok(database.users)
    }
    pub async fn get_from_database(&self, username: &str) -> Result<Option<User>, String>{ 
        let file = File::open(&self.connection.file)
            .map_err(|e| format!("Failed to open file: {}", e))?;
        let database: JsonBackendContent = serde_json::from_reader(&file).map_err(|e| {
            println!("Failed to parse JSON: {}", e); 
            format!("Error: {}", e)
        }).unwrap();
        if let Some(user) = database.users.iter().find(|user| user.username == username){
            Ok(Some(user.clone()))
        } else {
            Ok(None)
        }
    }
  
    
    pub async fn create_user_in_db(&self, user: CreateUserData) -> Result<User, String> {
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
        let password_hash = bcrypt::hash(user.password, bcrypt::DEFAULT_COST);

        let final_user = User {
            username: user.user,
            password_hash: Some(password_hash.unwrap()),
        };
        database.users.push(final_user.clone());
    
        let json = serde_json::to_string_pretty(&database)
            .map_err(|e| format!("Failed to serialize JSON: {}", e))?;
    
        let mut write_file = OpenOptions::new()
            .write(true)
            .truncate(true)
            .open(file_path)
            .map_err(|e| format!("Failed to re-open file for writing: {}", e))?;
    
        write_file.write_all(json.as_bytes()).map_err(|e| format!("Write error: {}", e))?;
    
        Ok(final_user)
    }
    
    fn clear_db(&self){
        let clear_file = OpenOptions::new()
            .write(true)
            .truncate(true)
            .read(true)
            .open(&self.connection.file)
            .map_err(|e| format!("Failed to open file: {}", e));
        if clear_file.is_err(){
            println!("{:#?}", clear_file);
        }
    }
    pub async fn remove_user_in_db(&self, user: RemoveUserData) -> Result<Option<User>, String> {
        Ok(None)
    }
}