
// use axum_login::AuthUser;
use bcrypt::{hash};

#[derive(Clone, Debug)]
pub struct User {
    pub username: String,
    pub password_hash: Option<String>,
}

pub fn retrive_user(username: String) -> Option<User> {
    let admin_user = std::env::var("ADMIN_USER").unwrap_or_default();
    let admin_password = std::env::var("ADMIN_PASSWORD").unwrap_or_default();

    if let Some(user) = get_from_database(username.clone()) {

        Some(user)
    } else if username == admin_user {
        let password_hash = bcrypt::hash(admin_password, bcrypt::DEFAULT_COST).ok();
        Some(User{
            username,
            password_hash
        })

    // } else if username == "testuser" {
    //     let password_hash = bcrypt::hash("password123", bcrypt::DEFAULT_COST).ok();
    //     Some(User {
    //         username,
    //         password_hash,
    //     })
    } else {
        None
    }
}

pub fn get_from_database(username: String) -> Option<User> {

    None
}