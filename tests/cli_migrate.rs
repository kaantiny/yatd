//! Integration tests for the migration system.

use assert_cmd::Command;
use tempfile::TempDir;

fn td() -> Command {
    Command::cargo_bin("td").unwrap()
}

fn init_tmp() -> TempDir {
    let tmp = TempDir::new().unwrap();
    td().arg("init").current_dir(&tmp).assert().success();
    tmp
}

#[test]
fn fresh_init_sets_latest_version() {
    let tmp = init_tmp();
    let conn = rusqlite::Connection::open(tmp.path().join(".td/tasks.db")).unwrap();
    let version: u32 = conn
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .unwrap();
    // Version should be 2 (migration 0001 + 0002).
    assert_eq!(version, 2);
}

#[test]
fn legacy_db_is_migrated_on_open() {
    let tmp = TempDir::new().unwrap();
    let td_dir = tmp.path().join(".td");
    std::fs::create_dir_all(&td_dir).unwrap();

    // Create a v0 database with the old schema (no effort column).
    let conn = rusqlite::Connection::open(td_dir.join("tasks.db")).unwrap();
    conn.execute_batch(
        "CREATE TABLE tasks (
            id TEXT PRIMARY KEY,
            title TEXT NOT NULL,
            description TEXT DEFAULT '',
            type TEXT DEFAULT 'task',
            priority INTEGER DEFAULT 2,
            status TEXT DEFAULT 'open',
            parent TEXT DEFAULT '',
            created TEXT NOT NULL,
            updated TEXT NOT NULL
        );
        CREATE TABLE labels (
            task_id TEXT, label TEXT,
            PRIMARY KEY (task_id, label),
            FOREIGN KEY (task_id) REFERENCES tasks(id)
        );
        CREATE TABLE blockers (
            task_id TEXT, blocker_id TEXT,
            PRIMARY KEY (task_id, blocker_id),
            FOREIGN KEY (task_id) REFERENCES tasks(id)
        );
        INSERT INTO tasks (id, title, created, updated)
            VALUES ('td-legacy', 'Old task', '2024-01-01T00:00:00Z', '2024-01-01T00:00:00Z');",
    )
    .unwrap();
    drop(conn);

    // Opening via td (list) should migrate and succeed.
    td().args(["--json", "list"])
        .current_dir(&tmp)
        .assert()
        .success();

    // Verify the task survived migration and got default effort.
    let out = td()
        .args(["--json", "show", "td-legacy"])
        .current_dir(&tmp)
        .output()
        .unwrap();
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(v["title"].as_str().unwrap(), "Old task");
    assert_eq!(v["effort"].as_i64().unwrap(), 2); // default medium

    // Verify version is now latest.
    let conn = rusqlite::Connection::open(td_dir.join("tasks.db")).unwrap();
    let version: u32 = conn
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .unwrap();
    assert_eq!(version, 2);
}

#[test]
fn effort_column_exists_after_init() {
    let tmp = init_tmp();
    let conn = rusqlite::Connection::open(tmp.path().join(".td/tasks.db")).unwrap();

    // Verify the effort column is present by inserting a row that sets it.
    conn.execute(
        "INSERT INTO tasks (id, title, effort, created, updated) VALUES ('td-test', 'Test', 3, '2024-01-01T00:00:00Z', '2024-01-01T00:00:00Z')",
        [],
    )
    .unwrap();

    let effort: i32 = conn
        .query_row("SELECT effort FROM tasks WHERE id = 'td-test'", [], |r| {
            r.get(0)
        })
        .unwrap();
    assert_eq!(effort, 3);
}
