use async_trait::async_trait;
// use sqlx::Error;
use std::error::Error;
use crate::Serialize;
use crate::Deserialize;

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
}

// #[async_trait]
pub trait UserDatabase {
    async fn retrieve_user(&self, username: String) -> Option<User>;
    async fn fetch_all(&self, item: &str) -> Result<Vec<User>, Box<dyn Error + Send + Sync>>;
    async fn get_from_database(&self, username: &str) -> Result<Option<User>, Box<dyn Error + Send + Sync>>;
    async fn create_user_in_db(&self, user: CreateUserData) -> Result<User, Box<dyn Error + Send + Sync>>;
    async fn remove_user_in_db(&self, user: RemoveUserData) -> Result<Option<User>, Box<dyn Error + Send + Sync>>;
}