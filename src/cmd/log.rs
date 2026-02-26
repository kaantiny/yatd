use anyhow::{bail, Result};
use std::path::Path;

use crate::db;

pub fn run(root: &Path, id: &str, message: &str, json: bool) -> Result<()> {
    let conn = db::open(root)?;

    if !db::task_exists(&conn, id)? {
        bail!("task {id} not found");
    }

    let timestamp = db::now_utc();
    conn.execute(
        "INSERT INTO task_logs (task_id, timestamp, body)
         VALUES (?1, ?2, ?3)",
        rusqlite::params![id, timestamp, message],
    )?;
    let log_id = conn.last_insert_rowid();
    conn.execute(
        "UPDATE tasks SET updated = ?1 WHERE id = ?2",
        rusqlite::params![db::now_utc(), id],
    )?;

    let entry = db::LogEntry {
        id: log_id,
        task_id: id.to_string(),
        timestamp,
        body: message.to_string(),
    };

    if json {
        println!("{}", serde_json::to_string(&entry)?);
    } else {
        println!("logged to {id}");
    }

    Ok(())
}
