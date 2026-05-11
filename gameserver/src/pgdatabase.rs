use sqlx::{SqlitePool, sqlite::SqlitePoolOptions};
use crate::databasespec::{Database, ServerIndex, ServerMetadata, Filters};
use std::collections::HashMap;

pub struct DbConn {
    
}
impl DbConn {
    pub async fn first_connection() -> Self {
        Self {
            
        }
    }
}



pub async fn open_pool(db_url: &str) -> Result<SqlitePool, sqlx::Error> {
    SqlitePoolOptions::new()
        .max_connections(5)
        .connect(db_url)
        .await
}

pub async fn ensure_db(pool: &SqlitePool) -> Result<(), sqlx::Error> {
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS config (
            key   TEXT PRIMARY KEY,
            value TEXT NOT NULL
        )"
    )
    .execute(pool)
    .await?;

    sqlx::query(
        "CREATE TABLE IF NOT EXISTS servers (
            name         TEXT PRIMARY KEY,
            location     TEXT NOT NULL,
            provider     TEXT NOT NULL,
            providertype TEXT NOT NULL,
            sandbox      BOOLEAN NOT NULL DEFAULT 0,
            start_keyword TEXT,
            stop_keyword  TEXT
        )"
    )
    .execute(pool)
    .await?;

    sqlx::query(
        "INSERT OR IGNORE INTO config (key, value) VALUES ('current_server', '')"
    )
    .execute(pool)
    .await?;

    sqlx::query(
        "INSERT OR IGNORE INTO config (key, value) VALUES ('filter', 'None')"
    )
    .execute(pool)
    .await?;

    Ok(())
}

pub async fn load_db(pool: &SqlitePool) -> Result<Database, sqlx::Error> {
    let current_server: String = sqlx::query_scalar(
        "SELECT value FROM config WHERE key = 'current_server'"
    )
    .fetch_one(pool)
    .await?;

    let filter_str: String = sqlx::query_scalar(
        "SELECT value FROM config WHERE key = 'filter'"
    )
    .fetch_one(pool)
    .await?;
    let filter = parse_filter(&filter_str);

    let rows = sqlx::query!(
        "SELECT name, location, provider, providertype, sandbox,
                start_keyword, stop_keyword
         FROM servers"
    )
    .fetch_all(pool)
    .await?;

    let server_index: HashMap<String, ServerIndex> = rows
        .into_iter()
        .map(|row| {
            let idx = ServerIndex::new(
                row.location,
                row.provider,
                row.providertype,
                row.sandbox,
                ServerMetadata {
                    start_keyword: row.start_keyword,
                    stop_keyword: row.stop_keyword,
                },
            );
            (row.name, idx)
        })
        .collect();

    Ok(Database { current_server, filter, server_index })
}

pub async fn save_db(pool: &SqlitePool, db: &Database) -> Result<(), sqlx::Error> {
    let mut tx = pool.begin().await?;

    sqlx::query!(
        "INSERT INTO config (key, value) VALUES ('current_server', ?1)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        db.current_server
    )
    .execute(&mut *tx)
    .await?;

    let filter_str = filter_to_str(&db.filter);
    sqlx::query!(
        "INSERT INTO config (key, value) VALUES ('filter', ?1)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        filter_str
    )
    .execute(&mut *tx)
    .await?;

    sqlx::query!("DELETE FROM servers")
        .execute(&mut *tx)
        .await?;

    for (name, idx) in &db.server_index {
        sqlx::query!(
            "INSERT INTO servers
                (name, location, provider, providertype, sandbox, start_keyword, stop_keyword)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            name,
            idx.location,
            idx.provider,
            idx.providertype,
            idx.sandbox,
            idx.server_metadata.start_keyword,
            idx.server_metadata.stop_keyword,
        )
        .execute(&mut *tx)
        .await?;
    }

    tx.commit().await?;
    Ok(())
}