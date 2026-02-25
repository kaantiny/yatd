use assert_cmd::Command;
use predicates::prelude::*;
use tempfile::TempDir;

fn td() -> Command {
    Command::cargo_bin("td").unwrap()
}

/// Initialise a temp directory and return it.
fn init_tmp() -> TempDir {
    let tmp = TempDir::new().unwrap();
    td().arg("init").current_dir(&tmp).assert().success();
    tmp
}

#[test]
fn create_prints_id_and_title() {
    let tmp = init_tmp();

    td().args(["create", "My first task"])
        .current_dir(&tmp)
        .assert()
        .success()
        .stdout(predicate::str::contains("My first task"));
}

#[test]
fn create_json_returns_task_object() {
    let tmp = init_tmp();

    td().args(["--json", "create", "Buy milk"])
        .current_dir(&tmp)
        .assert()
        .success()
        .stdout(predicate::str::contains(r#""title":"Buy milk"#))
        .stdout(predicate::str::contains(r#""status":"open"#))
        .stdout(predicate::str::contains(r#""priority":2"#));
}

#[test]
fn create_with_priority_and_type() {
    let tmp = init_tmp();

    td().args(["--json", "create", "Urgent bug", "-p", "1", "-t", "bug"])
        .current_dir(&tmp)
        .assert()
        .success()
        .stdout(predicate::str::contains(r#""priority":1"#))
        .stdout(predicate::str::contains(r#""type":"bug"#));
}

#[test]
fn create_with_description() {
    let tmp = init_tmp();

    td().args([
        "--json",
        "create",
        "Fix login",
        "-d",
        "The login page is broken",
    ])
    .current_dir(&tmp)
    .assert()
    .success()
    .stdout(predicate::str::contains("The login page is broken"));
}

#[test]
fn create_with_labels() {
    let tmp = init_tmp();

    td().args(["--json", "create", "Labelled task", "-l", "frontend,urgent"])
        .current_dir(&tmp)
        .assert()
        .success();

    // Verify labels are stored by checking the database directly.
    let conn = rusqlite::Connection::open(tmp.path().join(".td/tasks.db")).unwrap();
    let count: i64 = conn
        .query_row("SELECT COUNT(*) FROM labels", [], |r| r.get(0))
        .unwrap();
    assert_eq!(count, 2);
}

#[test]
fn create_requires_title() {
    let tmp = init_tmp();

    td().arg("create")
        .current_dir(&tmp)
        .assert()
        .failure()
        .stderr(predicate::str::contains("title required"));
}

#[test]
fn create_subtask_under_parent() {
    let tmp = init_tmp();

    // Create parent, extract its id.
    let parent_out = td()
        .args(["--json", "create", "Parent task"])
        .current_dir(&tmp)
        .output()
        .unwrap();
    let parent: serde_json::Value = serde_json::from_slice(&parent_out.stdout).unwrap();
    let parent_id = parent["id"].as_str().unwrap();

    // Create child under parent.
    let child_out = td()
        .args(["--json", "create", "Child task", "--parent", parent_id])
        .current_dir(&tmp)
        .output()
        .unwrap();
    let child: serde_json::Value = serde_json::from_slice(&child_out.stdout).unwrap();
    let child_id = child["id"].as_str().unwrap();

    // Child id should start with parent id.
    assert!(
        child_id.starts_with(parent_id),
        "child id '{child_id}' should start with parent id '{parent_id}'"
    );
    assert_eq!(child["parent"].as_str().unwrap(), parent_id);
}
