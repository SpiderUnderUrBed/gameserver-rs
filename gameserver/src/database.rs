use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

#[derive(Debug, Serialize, Deserialize, Default)]
struct Database {
    users: Vec<User>,
}

#[derive(Debug, Serialize, Deserialize)]
struct User {
    id: u32,
    name: String,
}

const DB_PATH: &str = "db.json";

fn load_db() -> Database {
    if !Path::new(DB_PATH).exists() {
        let db = Database::default();
        save_db(&db);
        return db;
    }
    let contents = fs::read_to_string(DB_PATH).expect("Failed to read DB file");
    serde_json::from_str(&contents).expect("Failed to parse DB")
}

fn save_db(db: &Database) {
    let contents = serde_json::to_string_pretty(db).expect("Failed to serialize");
    fs::write(DB_PATH, contents).expect("Failed to write DB file");
}


// This is from the json db template, remove in the future
// fn main() {
//     let mut db = load_db();

//     db.users.push(User { id: 1, name: "Alice".into() });
//     save_db(&db);

//     println!("{:#?}", db);
// }