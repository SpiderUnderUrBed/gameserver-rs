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
pub struct CreateUserData {
    pub user: String,
    pub password: String,
    pub jwt: String,
    pub user_perms: Vec<String>
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct RetrieveUser {
    pub user: String
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct RemoveUserData {
    pub user: String,
    pub jwt: String
}

#[cfg(any(feature = "full-stack", feature = "database"))]
#[derive(Clone, Debug, sqlx::FromRow, Serialize, Deserialize)]
pub struct User {
    pub username: String,
    pub password_hash: Option<String>,
}

#[cfg(all(not(feature = "full-stack"), not(feature = "database")))]
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct User {
    pub username: String,
    pub password_hash: Option<String>,
    pub user_perms: Vec<String>
}

// #[async_trait]
pub trait UserDatabase {
    async fn retrieve_user(&self, username: String) -> Option<User>;
    async fn fetch_all(&self, item: &str) -> Result<Vec<User>, Box<dyn Error + Send + Sync>>;
    async fn get_from_database(&self, username: &str) -> Result<Option<User>, Box<dyn Error + Send + Sync>>;
    async fn create_user_in_db(&self, user: CreateUserData) -> Result<StatusCode, Box<dyn Error + Send + Sync>>;
    async fn remove_user_in_db(&self, user: RemoveUserData) -> Result<StatusCode, Box<dyn Error + Send + Sync>>;
}