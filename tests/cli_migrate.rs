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
    // Version should be 5 (migration 0001 + 0002 + 0003 + 0004 + 0005).
    assert_eq!(version, 5);
}

#[test]
fn legacy_db_is_migrated_on_open() {
    let tmp = TempDir::new().unwrap();
    let td_dir = tmp.path().join(".td");
    std::fs::create_dir_all(&td_dir).unwrap();

    // Create a v0 database with the old schema (no effort column).
    let conn = rusqlite::Connection::open(td_dir.join("tasks.db")).unwrap();
    conn.execute_batch(
        "PRAGMA foreign_keys = OFF;
        CREATE TABLE tasks (
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
    assert_eq!(version, 5);
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

#[test]
fn blocker_fk_rejects_nonexistent_blocker_id() {
    let tmp = init_tmp();
    let conn = rusqlite::Connection::open(tmp.path().join(".td/tasks.db")).unwrap();
    conn.execute_batch("PRAGMA foreign_keys = ON").unwrap();

    conn.execute(
        "INSERT INTO tasks (id, title, created, updated) \
         VALUES ('td-real', 'Real task', '2024-01-01T00:00:00Z', '2024-01-01T00:00:00Z')",
        [],
    )
    .unwrap();

    // Inserting a blocker that references a nonexistent task should fail.
    let result = conn.execute(
        "INSERT INTO blockers (task_id, blocker_id) VALUES ('td-real', 'td-ghost')",
        [],
    );
    assert!(
        result.is_err(),
        "expected FK violation for nonexistent blocker_id"
    );
}

#[test]
fn labels_fk_cascades_on_task_delete() {
    let tmp = init_tmp();
    let conn = rusqlite::Connection::open(tmp.path().join(".td/tasks.db")).unwrap();
    conn.execute_batch("PRAGMA foreign_keys = ON").unwrap();

    conn.execute(
        "INSERT INTO tasks (id, title, created, updated) \
         VALUES ('td-labeled', 'Labeled task', '2024-01-01T00:00:00Z', '2024-01-01T00:00:00Z')",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO labels (task_id, label) VALUES ('td-labeled', 'urgent')",
        [],
    )
    .unwrap();

    conn.execute("DELETE FROM tasks WHERE id = 'td-labeled'", [])
        .unwrap();

    let label_count: i32 = conn
        .query_row(
            "SELECT COUNT(*) FROM labels WHERE task_id = 'td-labeled'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(
        label_count, 0,
        "labels should be deleted via ON DELETE CASCADE"
    );
}

#[test]
fn blockers_fk_cascades_on_task_delete() {
    let tmp = init_tmp();
    let conn = rusqlite::Connection::open(tmp.path().join(".td/tasks.db")).unwrap();
    conn.execute_batch("PRAGMA foreign_keys = ON").unwrap();

    conn.execute(
        "INSERT INTO tasks (id, title, created, updated) \
         VALUES ('td-a', 'Task A', '2024-01-01T00:00:00Z', '2024-01-01T00:00:00Z')",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO tasks (id, title, created, updated) \
         VALUES ('td-b', 'Task B', '2024-01-01T00:00:00Z', '2024-01-01T00:00:00Z')",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO tasks (id, title, created, updated) \
         VALUES ('td-c', 'Task C', '2024-01-01T00:00:00Z', '2024-01-01T00:00:00Z')",
        [],
    )
    .unwrap();

    // td-b appears as both task_id and blocker_id across these rows.
    conn.execute(
        "INSERT INTO blockers (task_id, blocker_id) VALUES ('td-b', 'td-a')",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO blockers (task_id, blocker_id) VALUES ('td-c', 'td-b')",
        [],
    )
    .unwrap();

    conn.execute("DELETE FROM tasks WHERE id = 'td-b'", [])
        .unwrap();

    let blocker_count: i32 = conn
        .query_row("SELECT COUNT(*) FROM blockers", [], |r| r.get(0))
        .unwrap();
    assert_eq!(
        blocker_count, 0,
        "rows referencing a deleted task should be deleted via ON DELETE CASCADE"
    );
}

#[test]
fn migration_cleans_dangling_blocker_ids() {
    let tmp = TempDir::new().unwrap();
    let td_dir = tmp.path().join(".td");
    std::fs::create_dir_all(&td_dir).unwrap();

    // Create a v2 database (pre-0003) with a dangling blocker_id.
    let conn = rusqlite::Connection::open(td_dir.join("tasks.db")).unwrap();
    conn.execute_batch(
        "PRAGMA foreign_keys = OFF;
        CREATE TABLE tasks (
            id TEXT PRIMARY KEY,
            title TEXT NOT NULL,
            description TEXT DEFAULT '',
            type TEXT DEFAULT 'task',
            priority INTEGER DEFAULT 2,
            status TEXT DEFAULT 'open',
            parent TEXT DEFAULT '',
            created TEXT NOT NULL,
            updated TEXT NOT NULL,
            effort INTEGER NOT NULL DEFAULT 2
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
            VALUES ('td-a', 'Task A', '2024-01-01T00:00:00Z', '2024-01-01T00:00:00Z');
        INSERT INTO tasks (id, title, created, updated)
            VALUES ('td-b', 'Task B', '2024-01-01T00:00:00Z', '2024-01-01T00:00:00Z');
        -- Valid blocker
        INSERT INTO blockers (task_id, blocker_id) VALUES ('td-a', 'td-b');
        -- Dangling blocker referencing a task that doesn't exist
        INSERT INTO blockers (task_id, blocker_id) VALUES ('td-a', 'td-gone');
        PRAGMA user_version = 2;",
    )
    .unwrap();
    drop(conn);

    // Running any command triggers migration.
    td().args(["--json", "list"])
        .current_dir(&tmp)
        .assert()
        .success();

    // The valid blocker should survive; the dangling one should be gone.
    let conn = rusqlite::Connection::open(td_dir.join("tasks.db")).unwrap();
    let count: i32 = conn
        .query_row(
            "SELECT COUNT(*) FROM blockers WHERE task_id = 'td-a'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(count, 1, "only the valid blocker should remain");

    let blocker: String = conn
        .query_row(
            "SELECT blocker_id FROM blockers WHERE task_id = 'td-a'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(blocker, "td-b");
}

#[test]
fn migration_cleans_dangling_labels() {
    let tmp = TempDir::new().unwrap();
    let td_dir = tmp.path().join(".td");
    std::fs::create_dir_all(&td_dir).unwrap();

    // Create a v4 database (pre-0005) with a dangling label row.
    let conn = rusqlite::Connection::open(td_dir.join("tasks.db")).unwrap();
    conn.execute_batch(
        "PRAGMA foreign_keys = OFF;
        CREATE TABLE tasks (
            id TEXT PRIMARY KEY,
            title TEXT NOT NULL,
            description TEXT DEFAULT '',
            type TEXT DEFAULT 'task',
            priority INTEGER DEFAULT 2,
            status TEXT DEFAULT 'open',
            parent TEXT DEFAULT '',
            created TEXT NOT NULL,
            updated TEXT NOT NULL,
            effort INTEGER NOT NULL DEFAULT 2
        );
        CREATE TABLE labels (
            task_id TEXT, label TEXT,
            PRIMARY KEY (task_id, label),
            FOREIGN KEY (task_id) REFERENCES tasks(id)
        );
        CREATE TABLE blockers (
            task_id TEXT, blocker_id TEXT,
            PRIMARY KEY (task_id, blocker_id),
            FOREIGN KEY (task_id) REFERENCES tasks(id),
            FOREIGN KEY (blocker_id) REFERENCES tasks(id)
        );
        CREATE TABLE task_logs (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            task_id TEXT NOT NULL,
            timestamp TEXT NOT NULL,
            body TEXT NOT NULL,
            FOREIGN KEY (task_id) REFERENCES tasks(id) ON DELETE CASCADE
        );
        INSERT INTO tasks (id, title, created, updated)
            VALUES ('td-real', 'Real task', '2024-01-01T00:00:00Z', '2024-01-01T00:00:00Z');
        INSERT INTO labels (task_id, label) VALUES ('td-real', 'kept');
        INSERT INTO labels (task_id, label) VALUES ('td-gone', 'orphan');
        PRAGMA user_version = 4;",
    )
    .unwrap();
    drop(conn);

    // Running any command triggers migration to v5.
    td().args(["--json", "list"])
        .current_dir(&tmp)
        .assert()
        .success();

    let conn = rusqlite::Connection::open(td_dir.join("tasks.db")).unwrap();
    let kept_count: i32 = conn
        .query_row(
            "SELECT COUNT(*) FROM labels WHERE task_id = 'td-real' AND label = 'kept'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(kept_count, 1, "valid label should survive migration");

    let orphan_count: i32 = conn
        .query_row(
            "SELECT COUNT(*) FROM labels WHERE task_id = 'td-gone'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(
        orphan_count, 0,
        "dangling label should be removed during migration"
    );
}
