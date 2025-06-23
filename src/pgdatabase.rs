
// use axum_login::AuthUser;
use bcrypt::{hash};
// use crate::Serialize;
// use crate::Deserialize;
use bcrypt::DEFAULT_COST;
use sqlx::{Pool, Postgres as SqlxPostgres};
// use sqlx::FromRow;
use crate::StatusCode;
use std::error::Error;
pub mod databasespec;
pub use databasespec::{User, Node, Element, CreateElementData, RemoveElementData, UserDatabase, NodesDatabase, DatabaseError, RetrieveUser};

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
    pub async fn clear_db(&self) -> Result<(), sqlx::Error> {
        let tables = [
            "users",
        ];

        let delete = format!("TRUNCATE TABLE {} RESTART IDENTITY CASCADE;", tables.join(", "));

        sqlx::query(&delete)
            .execute(&self.connection).await?;
        
        Ok(())
        // println!("Skipping this");
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
                password_hash,
                user_perms: vec![]
            })
        } else {
            None
        }
    }

    async fn fetch_all(&self) -> Result<Vec<User>, Box<dyn Error + Send + Sync>> {
        let users = sqlx::query_as::<_, User>("SELECT * FROM users")
        .fetch_all(&self.connection)
        .await?;

        Ok(users)
    }
    async fn edit_user_in_db(&self, element: CreateElementData) -> Result<StatusCode, Box<dyn Error + Send + Sync>> {
        if let Element::User { password, user, user_perms } = &element.element {
            if password.is_empty() {
                match sqlx::query(
                    r#"
                    UPDATE users
                    SET user_perms = $1,
                        updated_at = NOW()
                    WHERE username = $2
                    "#
                )
                .bind(&user_perms)
                .bind(&user)
                .execute(&self.connection)
                .await {
                    Ok(result) => {
                    },
                    Err(e) => {
                        return Err(Box::new(e));
                    }
                }
            } else {
                let password_hash = match bcrypt::hash(password, bcrypt::DEFAULT_COST) {
                    Ok(hash) => hash,
                    Err(e) => {
                        return Err(Box::new(e));
                    }
                };

                match sqlx::query(
                    r#"
                    UPDATE users
                    SET password_hash = $1,
                        user_perms = $2,
                        updated_at = NOW()
                    WHERE username = $3
                    "#
                )
                .bind(&password_hash)
                .bind(&user_perms)
                .bind(&user)
                .execute(&self.connection)
                .await {
                    Ok(result) => {
                    },
                    Err(e) => {
                        return Err(Box::new(e));
                    }
                }
            }

            Ok(StatusCode::CREATED)
        } else {
            Err(Box::new(DatabaseError(StatusCode::INTERNAL_SERVER_ERROR)))
        }
    }


    async fn create_user_in_db(&self, element: CreateElementData) -> Result<StatusCode, Box<dyn Error + Send + Sync>> {
        if let Element::User { password, user, user_perms } = element.element {
            let already_exists = self.get_from_database(&user).await?;
            println!("{:#?}", already_exists);
            if already_exists.is_some() {
                println!("Returning err");
                return Err(Box::new(DatabaseError(StatusCode::INTERNAL_SERVER_ERROR)));
            }
            if password.is_empty(){
                return Err(Box::new(DatabaseError(StatusCode::INTERNAL_SERVER_ERROR)));
            }


            let hashed = hash(&password, DEFAULT_COST).map_err(|e| {
                sqlx::Error::Protocol(e.to_string().into())
            })?;

            let final_user = sqlx::query_as::<_, User>("INSERT INTO users (username, password_hash, authcode, user_perms) VALUES ($1, $2, $3, $4) RETURNING *")
                .bind(user)
                .bind(hashed)
                .bind("0")
                .bind(user_perms)
            .fetch_one(&self.connection)
            .await?;

            Ok(StatusCode::CREATED)
        } else {
            Err(Box::new(DatabaseError(StatusCode::INTERNAL_SERVER_ERROR)))
        }
    }

    async fn get_from_database(&self, username: &str) -> Result<Option<User>, Box<dyn Error + Send + Sync>> { 
        let user = sqlx::query_as::<_, User>("SELECT * FROM users WHERE username = $1")
            .bind(username)
            .fetch_optional(&self.connection)
            .await?;
    
        Ok(user)
    }

    // pub fn remove_user_in_db(&self, user: RemoveElementData){
    //     //DELETE FROM users WHERE username = $1

    // }
    async fn remove_user_in_db(&self, element: RemoveElementData) -> Result<StatusCode, Box<dyn Error + Send + Sync>> {
        // if let Element::User { password, user, user_perms } = element.element {
            let final_user = sqlx::query_as::<_, User>("DELETE FROM users WHERE username = $1 RETURNING *")
                .bind(element.element)
                .fetch_optional(&self.connection)
                .await?;

            if final_user.is_some(){
                Ok(StatusCode::CREATED)
            } else {
                Err(Box::new(DatabaseError(StatusCode::INTERNAL_SERVER_ERROR)))
            }
        // } else {
        //     Err(Box::new(DatabaseError(StatusCode::INTERNAL_SERVER_ERROR)))
        // }
    }
}

impl NodesDatabase for Database {
    async fn retrieve_nodes(&self, nodename: String) -> Option<Node> {
        todo!()
    }
    async fn fetch_all_nodes(&self) -> Result<Vec<Node>, Box<dyn Error + Send + Sync>> {
        Ok(vec![])
    }
    async fn get_from_nodes_database(&self, nodename: &str) -> Result<Option<Node>, Box<dyn Error + Send + Sync>> {
        todo!()
    }
    async fn create_nodes_in_db(&self, node: CreateElementData) -> Result<StatusCode, Box<dyn Error + Send + Sync>> {
        todo!()
    }
    async fn remove_node_in_db(&self, node: RemoveElementData) -> Result<StatusCode, Box<dyn Error + Send + Sync>> {
        todo!()
    }
    async fn edit_node_in_db(&self, node: CreateElementData) -> Result<StatusCode, Box<dyn Error + Send + Sync>> {
        todo!()
    }
}