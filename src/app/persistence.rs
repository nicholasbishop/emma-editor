use super::App;
use anyhow::{anyhow, Result};
use fs_err as fs;
use rusqlite::Connection;
use std::path::PathBuf;

/// Name of the sqlite3 database file used to store cached data. This
/// data is separate from the config data (see config.rs). It can be
/// safely deleted, you'll just lose things like the list of open
/// buffers and pane tree layout after restarting the app.
const DB_NAME: &str = "emma.sqlite3";

/// Get the cache directory path, e.g. "~/.cache/emma".
fn cache_dir() -> Result<PathBuf> {
    Ok(dirs::cache_dir()
        .ok_or_else(|| anyhow!("cache dir unknown"))?
        .join("emma"))
}

impl App {
    pub fn persistence_store(&self) -> Result<()> {
        let cache_dir = cache_dir()?;

        // Try to create the directory. Ignore the error, it might
        // already exist.
        let _ = fs::create_dir_all(&cache_dir);

        // Open the database, creating it if necessary.
        let db_path = cache_dir.join(DB_NAME);
        let conn = Connection::open(db_path)?;

        // Create the tables if not already present.
        conn.execute(
            "CREATE TABLE IF NOT EXISTS kv (
                key   TEXT PRIMARY KEY,
                value TEXT
            )",
            (),
        )?;

        let json = serde_json::to_string(&self.pane_tree)?;
        conn.execute(
            "INSERT INTO kv (key, value) VALUES ('pane_tree', ?1)",
            (&json,),
        )?;

        Ok(())
    }

    pub fn persistence_load(&mut self) -> Result<()> {
        let cache_dir = cache_dir()?;
        let db_path = cache_dir.join(DB_NAME);
        let conn = Connection::open(db_path)?;

        let mut stmt =
            conn.prepare("SELECT value FROM kv WHERE key = 'pane_tree'")?;
        let pane_tree_json: String = stmt.query_row([], |row| row.get(0))?;

        // TODO self.pane_tree = serde_json::from_str(&pane_tree_json)?;

        Ok(())
    }
}
