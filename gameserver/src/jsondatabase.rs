use std::fs;
use std::path::Path;

use crate::databasespec::Database;

const DB_PATH: &str = "db.json";

pub fn ensure_db() -> Result<(), ()> {
    if !Path::new(DB_PATH).exists() {
        let db = Database::default();
        save_db(&db);
    }
    Ok(())
}
pub fn load_db() -> Database {
    if !Path::new(DB_PATH).exists() {
        let db = Database::default();
        save_db(&db);
        return db;
    }
    let contents = fs::read_to_string(DB_PATH).expect("Failed to read DB file");
    match serde_json::from_str(&contents) {
        Ok(db) => db,
        Err(e) => {
            eprintln!("Failed to parse DB (will be resetting): {}", e);
            let db = Database::default();
            save_db(&db);
            db
        }
    }
}

pub fn save_db(db: &Database) {
    let contents = serde_json::to_string_pretty(db).expect("Failed to serialize");
    fs::write(DB_PATH, contents).expect("Failed to write DB file");
}
