
use bcrypt::{hash};
//

use bcrypt::DEFAULT_COST;
use sqlx::{Pool, Postgres as SqlxPostgres};

use crate::StatusCode;
use std::error::Error;
pub mod databasespec;

pub use databasespec::{
    User, Node, Element, ModifyElementData, UserDatabase, NodesDatabase, 
    DatabaseError, RetrieveUser, Server, ServerDatabase, Button, ButtonsDatabase, Settings
};

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
            "users", "nodes", "servers", "buttons", "settings"
        ];

        let delete = format!("TRUNCATE TABLE {} RESTART IDENTITY CASCADE;", tables.join(", "));

        sqlx::query(&delete)
            .execute(&self.connection).await?;
        
        Ok(())
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
                user_perms: vec!["all".to_string()]
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
    
    async fn edit_user_in_db(&self, element: ModifyElementData) -> Result<StatusCode, Box<dyn Error + Send + Sync>> {
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
                    Ok(_result) => {},
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
                    Ok(_result) => {},
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

    async fn create_user_in_db(&self, element: ModifyElementData) -> Result<StatusCode, Box<dyn Error + Send + Sync>> {
        if let Element::User { password, user, user_perms } = element.element {
            let already_exists = self.get_from_database(&user).await?;
            if already_exists.is_some() {
                return Err(Box::new(DatabaseError(StatusCode::INTERNAL_SERVER_ERROR)));
            }
            if password.is_empty(){
                return Err(Box::new(DatabaseError(StatusCode::INTERNAL_SERVER_ERROR)));
            }

            let hashed = hash(&password, DEFAULT_COST).map_err(|e| {
                sqlx::Error::Protocol(e.to_string().into())
            })?;

            let _final_user = sqlx::query_as::<_, User>("INSERT INTO users (username, password_hash, authcode, user_perms) VALUES ($1, $2, $3, $4) RETURNING *")
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

    async fn remove_user_in_db(&self, element: ModifyElementData) -> Result<StatusCode, Box<dyn Error + Send + Sync>> {
        if let Element::User { user, .. } = element.element {
            let final_user = sqlx::query_as::<_, User>("DELETE FROM users WHERE username = $1 RETURNING *")
                .bind(user)
                .fetch_optional(&self.connection)
                .await?;

            if final_user.is_some(){
                Ok(StatusCode::CREATED)
            } else {
                Err(Box::new(DatabaseError(StatusCode::INTERNAL_SERVER_ERROR)))
            }
        } else {
            Err(Box::new(DatabaseError(StatusCode::INTERNAL_SERVER_ERROR)))
        }
    }
}

impl NodesDatabase for Database {
    async fn retrieve_nodes(&self, nodename: String) -> Option<Node> {
        match self.get_from_nodes_database(&nodename).await {
            Ok(node) => node,
            Err(_) => None
        }
    }
    
    async fn fetch_all_nodes(&self) -> Result<Vec<Node>, Box<dyn Error + Send + Sync>> {
        let nodes = sqlx::query_as::<_, Node>("SELECT * FROM nodes")
            .fetch_all(&self.connection)
            .await?;
        Ok(nodes)
    }
    
    async fn get_from_nodes_database(&self, nodename: &str) -> Result<Option<Node>, Box<dyn Error + Send + Sync>> {
        let node = sqlx::query_as::<_, Node>("SELECT * FROM nodes WHERE nodename = $1")
            .bind(nodename)
            .fetch_optional(&self.connection)
            .await?;
        Ok(node)
    }
    
    async fn create_nodes_in_db(&self, node: ModifyElementData) -> Result<StatusCode, Box<dyn Error + Send + Sync>> {
        if let Element::Node(node_data) = node.element {
            
            let existing = self.get_from_nodes_database(&node_data.nodename).await?;
            if existing.is_some() {
                return Err(Box::new(DatabaseError(StatusCode::INTERNAL_SERVER_ERROR)));
            }

            let _result = sqlx::query_as::<_, Node>(
                "INSERT INTO nodes (nodename, ip, nodetype, nodestatus) VALUES ($1, $2, $3, $4) RETURNING *"
            )
            .bind(&node_data.nodename)
            .bind(&node_data.ip)
            .bind(&node_data.nodetype)
            .bind(&node_data.nodestatus)
            .fetch_one(&self.connection)
            .await?;

            Ok(StatusCode::CREATED)
        } else {
            Err(Box::new(DatabaseError(StatusCode::INTERNAL_SERVER_ERROR)))
        }
    }
    
    async fn remove_node_in_db(&self, node: ModifyElementData) -> Result<StatusCode, Box<dyn Error + Send + Sync>> {
        if let Element::Node(node_data) = node.element {
            let result = sqlx::query("DELETE FROM nodes WHERE nodename = $1")
                .bind(&node_data.nodename)
                .execute(&self.connection)
                .await?;

            if result.rows_affected() > 0 {
                Ok(StatusCode::CREATED)
            } else {
                Err(Box::new(DatabaseError(StatusCode::INTERNAL_SERVER_ERROR)))
            }
        } else {
            Err(Box::new(DatabaseError(StatusCode::INTERNAL_SERVER_ERROR)))
        }
    }
    
    async fn edit_node_in_db(&self, node: ModifyElementData) -> Result<StatusCode, Box<dyn Error + Send + Sync>> {
        if let Element::Node(node_data) = node.element {
            let result = sqlx::query(
                r#"
                UPDATE nodes 
                SET ip = $1, nodetype = $2, nodestatus = $3, updated_at = NOW()
                WHERE nodename = $4
                "#
            )
            .bind(&node_data.ip)
            .bind(&node_data.nodetype)
            .bind(&node_data.nodestatus)
            .bind(&node_data.nodename)
            .execute(&self.connection)
            .await?;

            if result.rows_affected() > 0 {
                Ok(StatusCode::CREATED)
            } else {
                Err(Box::new(DatabaseError(StatusCode::INTERNAL_SERVER_ERROR)))
            }
        } else {
            Err(Box::new(DatabaseError(StatusCode::INTERNAL_SERVER_ERROR)))
        }
    }
}

impl ServerDatabase for Database {
    async fn retrieve_server(&self, servername: String) -> Option<Server> {
        match self.get_from_servers_database(&servername).await {
            Ok(server) => server,
            Err(_) => None
        }
    }
    
    async fn fetch_all_servers(&self) -> Result<Vec<Server>, Box<dyn Error + Send + Sync>> {
        let servers = sqlx::query_as::<_, Server>("SELECT * FROM servers")
            .fetch_all(&self.connection)
            .await?;
        Ok(servers)
    }
    
    async fn get_from_servers_database(&self, servername: &str) -> Result<Option<Server>, Box<dyn Error + Send + Sync>> {
        let server = sqlx::query_as::<_, Server>("SELECT * FROM servers WHERE servername = $1")
            .bind(servername)
            .fetch_optional(&self.connection)
            .await?;
        Ok(server)
    }
    
    async fn create_server_in_db(&self, element: ModifyElementData) -> Result<StatusCode, Box<dyn Error + Send + Sync>> {
        if let Element::Server(server) = element.element {
            
            let existing = self.get_from_servers_database(&server.servername).await?;
            if existing.is_some() {
                return Err(Box::new(DatabaseError(StatusCode::INTERNAL_SERVER_ERROR)));
            }

            let _result = sqlx::query_as::<_, Server>(
                "INSERT INTO servers (servername, provider, providertype, location) VALUES ($1, $2, $3, $4) RETURNING *"
            )
            .bind(&server.servername)
            .bind(&server.provider)
            .bind(&server.providertype)
            .bind(&server.location)
            .fetch_one(&self.connection)
            .await?;

            Ok(StatusCode::CREATED)
        } else {
            Err(Box::new(DatabaseError(StatusCode::INTERNAL_SERVER_ERROR)))
        }
    }
    
    async fn remove_server_in_db(&self, element: ModifyElementData) -> Result<StatusCode, Box<dyn Error + Send + Sync>> {
        if let Element::Server(server) = element.element {
            let result = sqlx::query("DELETE FROM servers WHERE servername = $1")
                .bind(&server.servername)
                .execute(&self.connection)
                .await?;

            if result.rows_affected() > 0 {
                Ok(StatusCode::CREATED)
            } else {
                Err(Box::new(DatabaseError(StatusCode::INTERNAL_SERVER_ERROR)))
            }
        } else {
            Err(Box::new(DatabaseError(StatusCode::INTERNAL_SERVER_ERROR)))
        }
    }
    
    async fn edit_server_in_db(&self, element: ModifyElementData) -> Result<StatusCode, Box<dyn Error + Send + Sync>> {
        if let Element::Server(server) = element.element {
            let result = sqlx::query(
                r#"
                UPDATE servers 
                SET provider = $1, providertype = $2, location = $3, updated_at = NOW()
                WHERE servername = $4
                "#
            )
            .bind(&server.provider)
            .bind(&server.providertype)
            .bind(&server.location)
            .bind(&server.servername)
            .execute(&self.connection)
            .await?;

            if result.rows_affected() > 0 {
                Ok(StatusCode::CREATED)
            } else {
                Err(Box::new(DatabaseError(StatusCode::INTERNAL_SERVER_ERROR)))
            }
        } else {
            Err(Box::new(DatabaseError(StatusCode::INTERNAL_SERVER_ERROR)))
        }
    }
}

impl ButtonsDatabase for Database {
    async fn retrieve_buttons(&self, name: String) -> Option<Button> {
        match self.get_from_buttons_database(&name).await {
            Ok(button) => button,
            Err(_) => None
        }
    }
    
    async fn fetch_all_buttons(&self) -> Result<Vec<Button>, Box<dyn Error + Send + Sync>> {
        let buttons = sqlx::query_as::<_, Button>("SELECT * FROM buttons ORDER BY name")
            .fetch_all(&self.connection)
            .await?;
        Ok(buttons)
    }
    
    async fn get_from_buttons_database(&self, name: &str) -> Result<Option<Button>, Box<dyn Error + Send + Sync>> {
        let button = sqlx::query_as::<_, Button>("SELECT * FROM buttons WHERE name = $1")
            .bind(name)
            .fetch_optional(&self.connection)
            .await?;
        Ok(button)
    }
    
    async fn toggle_button_state(&self) -> Result<bool, Box<dyn Error + Send + Sync>> {
        let result: (bool,) = sqlx::query_as("SELECT toggled_default_buttons FROM settings LIMIT 1")
            .fetch_optional(&self.connection)
            .await?
            .unwrap_or((false,));
        Ok(result.0)
    }
    
    async fn toggle_default_buttons(&self) -> Result<StatusCode, Box<dyn Error + Send + Sync>> {
        
        let current_state = self.toggle_button_state().await?;
        let new_state = !current_state;
        
        
        sqlx::query("UPDATE settings SET toggled_default_buttons = $1")
            .bind(new_state)
            .execute(&self.connection)
            .await?;

        
        if new_state {
            self.reset_buttons().await?;
        }
        
        Ok(StatusCode::CREATED)
    }
    
    async fn reset_buttons(&self) -> Result<StatusCode, Box<dyn Error + Send + Sync>> {
        
        sqlx::query("DELETE FROM buttons")
            .execute(&self.connection)
            .await?;
        
        
        let default_buttons = vec![
            ("Filebrowser", "", "default"),
            ("Statistics", "", "default"),
            ("Workflows", "", "default"),
            ("Intergrations", "", "default"),
            ("Backups", "", "default"),
            ("Settings", "", "default"),
        ];
        
        for (name, link, button_type) in default_buttons {
            sqlx::query("INSERT INTO buttons (name, link, type) VALUES ($1, $2, $3)")
                .bind(name)
                .bind(link)
                .bind(button_type)
                .execute(&self.connection)
                .await?;
        }
        
        Ok(StatusCode::CREATED)
    }
    
    async fn edit_button_in_db(&self, element: ModifyElementData) -> Result<StatusCode, Box<dyn Error + Send + Sync>> {
        if let Element::Button(button) = element.element {
            let result = sqlx::query(
                r#"
                UPDATE buttons 
                SET link = $1, type = 'custom', updated_at = NOW()
                WHERE LOWER(name) = LOWER($2)
                "#
            )
            .bind(&button.link)
            .bind(&button.name)
            .execute(&self.connection)
            .await?;

            if result.rows_affected() > 0 {
                Ok(StatusCode::CREATED)
            } else {
                Err(Box::new(DatabaseError(StatusCode::INTERNAL_SERVER_ERROR)))
            }
        } else {
            Err(Box::new(DatabaseError(StatusCode::INTERNAL_SERVER_ERROR)))
        }
    }
}