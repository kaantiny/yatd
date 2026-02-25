use anyhow::{bail, Result};
use rusqlite::Connection;
use serde::Serialize;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::SystemTime;

const TD_DIR: &str = ".td";
const DB_FILE: &str = "tasks.db";

/// A task record.
#[derive(Debug, Serialize)]
pub struct Task {
    pub id: String,
    pub title: String,
    pub description: String,
    #[serde(rename = "type")]
    pub task_type: String,
    pub priority: i32,
    pub status: String,
    pub parent: String,
    pub created: String,
    pub updated: String,
}

static ID_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Generate a short unique ID like `td-a1b2c3`.
pub fn gen_id() -> String {
    let mut hasher = DefaultHasher::new();
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_nanos()
        .hash(&mut hasher);
    std::process::id().hash(&mut hasher);
    ID_COUNTER.fetch_add(1, Ordering::Relaxed).hash(&mut hasher);
    format!("td-{:06x}", hasher.finish() & 0xffffff)
}

/// A task with its labels and blockers.
#[derive(Debug, Serialize)]
pub struct TaskDetail {
    #[serde(flatten)]
    pub task: Task,
    pub labels: Vec<String>,
    pub blockers: Vec<String>,
}

/// Current UTC time in ISO 8601 format.
pub fn now_utc() -> String {
    chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string()
}

/// Read a single `Task` row from a query result.
pub fn row_to_task(row: &rusqlite::Row) -> rusqlite::Result<Task> {
    Ok(Task {
        id: row.get("id")?,
        title: row.get("title")?,
        description: row.get("description")?,
        task_type: row.get("type")?,
        priority: row.get("priority")?,
        status: row.get("status")?,
        parent: row.get("parent")?,
        created: row.get("created")?,
        updated: row.get("updated")?,
    })
}

/// Load labels for a task.
pub fn load_labels(conn: &Connection, task_id: &str) -> Result<Vec<String>> {
    let mut stmt = conn.prepare("SELECT label FROM labels WHERE task_id = ?1")?;
    let labels = stmt
        .query_map([task_id], |r| r.get(0))?
        .collect::<rusqlite::Result<Vec<String>>>()?;
    Ok(labels)
}

/// Load blockers for a task.
pub fn load_blockers(conn: &Connection, task_id: &str) -> Result<Vec<String>> {
    let mut stmt = conn.prepare("SELECT blocker_id FROM blockers WHERE task_id = ?1")?;
    let blockers = stmt
        .query_map([task_id], |r| r.get(0))?
        .collect::<rusqlite::Result<Vec<String>>>()?;
    Ok(blockers)
}

/// Load a full task with labels and blockers.
pub fn load_task_detail(conn: &Connection, id: &str) -> Result<TaskDetail> {
    let task = conn.query_row(
        "SELECT id, title, description, type, priority, status, parent, created, updated
         FROM tasks WHERE id = ?1",
        [id],
        row_to_task,
    )?;
    let labels = load_labels(conn, id)?;
    let blockers = load_blockers(conn, id)?;
    Ok(TaskDetail {
        task,
        labels,
        blockers,
    })
}

const SCHEMA: &str = "
CREATE TABLE tasks (
    id          TEXT PRIMARY KEY,
    title       TEXT NOT NULL,
    description TEXT DEFAULT '',
    type        TEXT DEFAULT 'task',
    priority    INTEGER DEFAULT 2,
    status      TEXT DEFAULT 'open',
    parent      TEXT DEFAULT '',
    created     TEXT NOT NULL,
    updated     TEXT NOT NULL
);

CREATE TABLE labels (
    task_id TEXT,
    label   TEXT,
    PRIMARY KEY (task_id, label),
    FOREIGN KEY (task_id) REFERENCES tasks(id)
);

CREATE TABLE blockers (
    task_id    TEXT,
    blocker_id TEXT,
    PRIMARY KEY (task_id, blocker_id),
    FOREIGN KEY (task_id) REFERENCES tasks(id)
);

CREATE INDEX idx_status   ON tasks(status);
CREATE INDEX idx_priority ON tasks(priority);
CREATE INDEX idx_parent   ON tasks(parent);
";

/// Walk up from `start` looking for a `.td/` directory.
pub fn find_root(start: &Path) -> Result<PathBuf> {
    let mut dir = start.to_path_buf();
    loop {
        if dir.join(TD_DIR).is_dir() {
            return Ok(dir);
        }
        if !dir.pop() {
            bail!("not initialized. Run 'td init'");
        }
    }
}

/// Create the `.td/` directory and initialise the database schema.
pub fn init(root: &Path) -> Result<Connection> {
    let td = root.join(TD_DIR);
    std::fs::create_dir_all(&td)?;
    let conn = Connection::open(td.join(DB_FILE))?;
    conn.execute_batch("PRAGMA foreign_keys = ON;")?;
    conn.execute_batch(SCHEMA)?;
    Ok(conn)
}

/// Open an existing database.
pub fn open(root: &Path) -> Result<Connection> {
    let path = root.join(TD_DIR).join(DB_FILE);
    if !path.exists() {
        bail!("not initialized. Run 'td init'");
    }
    let conn = Connection::open(path)?;
    conn.execute_batch("PRAGMA foreign_keys = ON;")?;
    Ok(conn)
}

/// Return the path to the `.td/` directory under `root`.
pub fn td_dir(root: &Path) -> PathBuf {
    root.join(TD_DIR)
}
