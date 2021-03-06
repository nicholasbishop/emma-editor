use {
    crate::{
        buffer::{BufferId, BufferKind, Embuf, RestoreInfo},
        highlight::HighlightRequest,
        pane_tree::{PaneTree, PaneTreeSerdeNode},
        App,
    },
    anyhow::{anyhow, Error},
    crossbeam_channel::Sender,
    fehler::throws,
    log::error,
    std::{ffi::OsString, fs, os::unix::ffi::OsStringExt, path::PathBuf},
};

fn open_db() -> Result<rusqlite::Connection, Error> {
    let dir = dirs::cache_dir()
        .ok_or_else(|| anyhow!("cache dir not found"))?
        .join("emma");
    if !dir.exists() {
        fs::create_dir_all(&dir)?;
    }
    let path = dir.join("state.sqlite3");
    let conn = rusqlite::Connection::open(&path)?;
    Ok(conn)
}

pub fn init_db() -> Result<(), Error> {
    let conn = open_db()?;
    // TODO: upgrades
    conn.execute(
        "CREATE TABLE IF NOT EXISTS open_buffers (
                      buffer_id TEXT PRIMARY KEY,
                      name TEXT,
                      path BLOB,
                      kind TEXT)",
        rusqlite::NO_PARAMS,
    )?;
    conn.execute(
        "CREATE TABLE IF NOT EXISTS layout_history (
                      id INTEGER PRIMARY KEY,
                      json TEXT)",
        rusqlite::NO_PARAMS,
    )?;
    Ok(())
}

#[throws]
pub fn restore_embufs(
    highlight_request_sender: Sender<HighlightRequest>,
) -> Vec<Embuf> {
    let conn = open_db()?;
    let mut stmt =
        conn.prepare("SELECT buffer_id, name, path, kind FROM open_buffers")?;

    let v = stmt
        .query_and_then(rusqlite::NO_PARAMS, |b| {
            let id: BufferId = b.get(0)?;
            let name: String = b.get(1)?;
            let path: Vec<u8> = b.get(2)?;
            let kind: String = b.get(3)?;
            Ok(Embuf::restore(
                RestoreInfo {
                    id,
                    name,
                    path: PathBuf::from(OsString::from_vec(path)),
                    kind: BufferKind::from_str(&kind).ok_or_else(|| {
                        anyhow!("invalid buffer kind: {}", kind)
                    })?,
                },
                highlight_request_sender.clone(),
            )?)
        })?
        .collect::<Vec<Result<Embuf, Error>>>();
    let mut embufs: Vec<Embuf> = Vec::new();
    for item in v {
        match item {
            Ok(b) => embufs.push(b),
            Err(err) => {
                error!("failed to load buffer: {:#}", err);
            }
        }
    }
    embufs
}

#[throws]
pub fn persist_app(app: &App) {
    let mut conn = open_db()?;

    persist_embufs(&mut conn, &app.buffers).unwrap();
    persist_layout_history(&mut conn, &app.pane_tree).unwrap();
}

#[throws]
fn persist_embufs(conn: &mut rusqlite::Connection, embufs: &[Embuf]) {
    let tx = conn.transaction()?;

    tx.execute("DELETE FROM open_buffers", rusqlite::NO_PARAMS)?;

    // TODO: batch
    for embuf in embufs {
        tx.execute(
            "INSERT INTO open_buffers (buffer_id, name, path, kind)
                  VALUES (?1, ?2, ?3, ?4)",
            rusqlite::params![
                embuf.buffer_id(),
                embuf.name(),
                embuf.path().into_os_string().into_vec(),
                embuf.kind().to_str()
            ],
        )?;
    }

    tx.commit()?;
}

#[throws]
fn persist_layout_history(
    conn: &mut rusqlite::Connection,
    pane_tree: &PaneTree,
) {
    let tx = conn.transaction()?;

    tx.execute("DELETE FROM layout_history", rusqlite::NO_PARAMS)?;
    // TODO: store recent layout history. For now just store the
    // current layout.
    let json = serde_json::to_string(&pane_tree.serialize())?;
    tx.execute(
        "INSERT INTO layout_history (id, json)
         VALUES (?1, ?2)",
        rusqlite::params![1, json],
    )?;

    tx.commit()?;
}

#[throws]
pub fn get_layout_history() -> Vec<PaneTreeSerdeNode> {
    let conn = open_db()?;
    let mut stmt = conn.prepare("SELECT id, json FROM layout_history")?;

    let v = stmt
        .query_and_then(rusqlite::NO_PARAMS, |b| {
            let _id: i64 = b.get(0)?;
            let json: String = b.get(1)?;
            let node = serde_json::from_str(&json)?;
            Ok(node)
        })?
        .collect::<Vec<Result<PaneTreeSerdeNode, Error>>>();
    let mut layouts = Vec::new();
    for item in v {
        match item {
            Ok(l) => layouts.push(l),
            Err(err) => {
                error!("failed to load layout: {:#}", err);
            }
        }
    }
    layouts
}
