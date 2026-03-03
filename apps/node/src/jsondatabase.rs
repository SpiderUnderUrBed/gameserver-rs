use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::Path;

use crate::databasespec::{self, Database};

use databasespec::ServerIndex;


const DB_PATH: &str = "db.json";

pub fn load_db() -> Database {
    if !Path::new(DB_PATH).exists() {
        let db = Database::default();
        save_db(&db);
        return db;
    }
    let contents = fs::read_to_string(DB_PATH).expect("Failed to read DB file");
    serde_json::from_str(&contents).expect("Failed to parse DB")
}

pub fn save_db(db: &Database) {
    let contents = serde_json::to_string_pretty(db).expect("Failed to serialize");
    fs::write(DB_PATH, contents).expect("Failed to write DB file");
}
