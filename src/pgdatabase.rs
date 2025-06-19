
// use axum_login::AuthUser;
use bcrypt::{hash};
// use crate::Serialize;
// use crate::Deserialize;
use bcrypt::DEFAULT_COST;
use sqlx::{Pool, Postgres as SqlxPostgres};
// use sqlx::FromRow;

use std::error::Error;
pub mod databasespec;
pub use databasespec::{User, CreateUserData, RemoveUserData, UserDatabase};

// } else if username == "testuser" {
//     let password_hash = bcrypt::hash("password123", bcrypt::DEFAULT_COST).ok();
//     Some(User {
//         username,
//         password_hash,
//     })

#[derive(Clone)]
pub struct Database {
    connection: Pool<SqlxPostgres>,
}

impl Database {
    pub fn new(connection: Option<Pool<SqlxPostgres>>) -> Database {
        Database {
            connection: connection.unwrap(),
        }
    }
}
impl UserDatabase for Database { 
    async fn retrieve_user(&self, username: String) -> Option<User> {
        let enable_admin_user = std::env::var("ENABLE_ADMIN_USER").unwrap_or_default() == "true";
        let admin_user = std::env::var("ADMIN_USER").unwrap_or_default();
        let admin_password = std::env::var("ADMIN_PASSWORD").unwrap_or_default();

        if let Ok(Some(user)) = self.get_from_database(&username.clone()).await {

            Some(user)
        } else if username == admin_user && enable_admin_user {
            let password_hash = bcrypt::hash(admin_password, bcrypt::DEFAULT_COST).ok();
            Some(User{
                username,
                password_hash
            })
        } else {
            None
        }
    }

    async fn fetch_all(&self, item: &str) -> Result<Vec<User>, Box<dyn Error + Send + Sync>> {
        let users = sqlx::query_as::<_, User>(&format!("SELECT * FROM {}", item))
        .fetch_all(&self.connection)
        .await?;

        Ok(users)
    }

    async fn get_from_database(&self, username: &str) -> Result<Option<User>, Box<dyn Error + Send + Sync>> { 
        let user = sqlx::query_as::<_, User>("SELECT * FROM users WHERE username = $1")
            .bind(username)
            .fetch_optional(&self.connection)
            .await?;
    
        Ok(user)
    }

    async fn create_user_in_db(&self, user: CreateUserData) -> Result<User, Box<dyn Error + Send + Sync>> {
        let hashed = hash(&user.password, DEFAULT_COST).map_err(|e| {
            sqlx::Error::Protocol(e.to_string().into())
        })?;
        let user = sqlx::query_as::<_, User>("INSERT INTO users (username, password_hash, authcode) VALUES ($1, $2, $3) RETURNING *")
            .bind(user.user)
            .bind(hashed)
            .bind("0")
        .fetch_one(&self.connection)
        .await?;

        Ok(user)
    }
    // pub fn remove_user_in_db(&self, user: RemoveUserData){
    //     //DELETE FROM users WHERE username = $1

    // }
    async fn remove_user_in_db(&self, user: RemoveUserData) -> Result<Option<User>, Box<dyn Error + Send + Sync>> {
        let user = sqlx::query_as::<_, User>("DELETE FROM users WHERE username = $1 RETURNING *")
            .bind(user.user)
        .fetch_optional(&self.connection)
        .await?;

        Ok(user)
    }
}