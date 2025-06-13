use serde::Deserialize;
use serde::Serialize;

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

#[derive(Default, Clone)]
pub struct JsonBackend {
    
}

#[derive(Clone, Default)]
pub struct Database {
    connection: JsonBackend
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
            Some(User{
                username,
                password_hash
            })
        } else {
            None
        }

    }
    pub async fn fetch_all(&self, item: &str) -> Result<Vec<User>, String>{
        Ok(vec![])
    }
    pub async fn get_from_database(&self, username: &str) -> Result<Option<User>, String>{ 
        Ok(None)
    }
    pub async fn create_user_in_db(&self, user: CreateUserData) -> Result<User, String> {
        Ok(User {
            username: user.user,
            password_hash: Some(user.password),
        })
    }
    pub async fn remove_user_in_db(&self, user: RemoveUserData) -> Result<Option<User>, String> {
        Ok(None)
    }
}