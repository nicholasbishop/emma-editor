use super::AppState;
use crate::buffer::{BufferId, CursorMap};
use anyhow::{anyhow, Result};
use fs_err as fs;
use rusqlite::Connection;
use std::ffi::OsStr;
use std::os::unix::ffi::OsStrExt;
use std::path::PathBuf;
use tracing::error;

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

#[derive(Debug)]
pub struct PersistedBuffer {
    pub buffer_id: BufferId,
    pub path: Option<PathBuf>,
    pub cursors: CursorMap,
}

impl AppState {
    pub fn persistence_store(&self) -> Result<()> {
        if !self.is_persistence_enabled {
            return Ok(());
        }

        let cache_dir = cache_dir()?;

        // Try to create the directory. Ignore the error, it might
        // already exist.
        let _ = fs::create_dir_all(&cache_dir);

        // Open the database, creating it if necessary.
        let db_path = cache_dir.join(DB_NAME);
        let mut conn = Connection::open(db_path)?;

        // Create the tables if not already present.
        conn.execute(
            "CREATE TABLE IF NOT EXISTS kv (
                key   TEXT PRIMARY KEY,
                value TEXT
            )",
            (),
        )?;
        conn.execute(
            "CREATE TABLE IF NOT EXISTS buffers (
                buffer_id TEXT PRIMARY KEY,
                path BLOB,
                cursors TEXT
            )",
            (),
        )?;

        let json = serde_json::to_string(&self.pane_tree)?;
        conn.execute(
            "REPLACE INTO kv (key, value) VALUES ('pane_tree', ?1)",
            (&json,),
        )?;

        let tx = conn.transaction()?;
        tx.execute("DELETE FROM buffers", ())?;
        for (buffer_id, buffer) in &self.buffers {
            if buffer_id.is_minibuf() {
                continue;
            }
            let cursors = serde_json::to_string(buffer.cursors())?;
            tx.execute(
                "INSERT INTO buffers (buffer_id, path, cursors) VALUES (?1, ?2, ?3)",
                (
                    buffer_id.as_str(),
                    buffer.path().map(|p| p.as_os_str().as_bytes()),
                    cursors
                ),
            )?;
        }
        tx.commit()?;

        Ok(())
    }

    pub fn load_persisted_buffers() -> Result<Vec<PersistedBuffer>> {
        // TODO: dedup
        let cache_dir = cache_dir()?;
        let db_path = cache_dir.join(DB_NAME);
        let conn = Connection::open(db_path)?;

        let mut stmt =
            conn.prepare("SELECT buffer_id, path, cursors FROM buffers")?;
        let iter = stmt.query_map([], |row| {
            let path: Option<Vec<u8>> = row.get(1)?;
            let cursors: String = row.get(2)?;
            let cursors = match serde_json::from_str(&cursors) {
                Ok(c) => c,
                Err(err) => {
                    error!("failed to deserialize buffer cursors: {err}");
                    Default::default()
                }
            };
            Ok(PersistedBuffer {
                buffer_id: BufferId::from_string(row.get(0)?),
                path: path.map(|path| PathBuf::from(OsStr::from_bytes(&path))),
                cursors,
            })
        })?;
        Ok(iter.collect::<Result<_, _>>()?)
    }

    /// Load JSON that describes the pane tree.
    pub fn load_persisted_pane_tree() -> Result<String> {
        // TODO: dedup
        let cache_dir = cache_dir()?;
        let db_path = cache_dir.join(DB_NAME);
        let conn = Connection::open(db_path)?;

        let mut stmt =
            conn.prepare("SELECT value FROM kv WHERE key = 'pane_tree'")?;
        let pane_tree_json: String = stmt.query_row([], |row| row.get(0))?;

        Ok(pane_tree_json)
    }
}
