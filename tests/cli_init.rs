use assert_cmd::Command;
use predicates::prelude::*;
use tempfile::TempDir;

fn td() -> Command {
    Command::cargo_bin("td").unwrap()
}

#[test]
fn init_creates_td_directory_and_database() {
    let tmp = TempDir::new().unwrap();

    td().arg("init")
        .current_dir(&tmp)
        .assert()
        .success()
        .stderr(predicate::str::contains("initialized .td/"));

    assert!(tmp.path().join(".td").is_dir());
    assert!(tmp.path().join(".td/tasks.db").is_file());
}

#[test]
fn init_creates_schema_with_expected_tables() {
    let tmp = TempDir::new().unwrap();

    td().arg("init").current_dir(&tmp).assert().success();

    let conn = rusqlite::Connection::open(tmp.path().join(".td/tasks.db")).unwrap();

    // Verify all three tables exist by querying sqlite_master.
    let tables: Vec<String> = conn
        .prepare("SELECT name FROM sqlite_master WHERE type='table' ORDER BY name")
        .unwrap()
        .query_map([], |row| row.get(0))
        .unwrap()
        .map(|r| r.unwrap())
        .collect();

    assert!(tables.contains(&"tasks".to_string()));
    assert!(tables.contains(&"labels".to_string()));
    assert!(tables.contains(&"blockers".to_string()));
}

#[test]
fn init_fails_when_already_initialized() {
    let tmp = TempDir::new().unwrap();

    td().arg("init").current_dir(&tmp).assert().success();

    td().arg("init")
        .current_dir(&tmp)
        .assert()
        .failure()
        .stderr(predicate::str::contains("already initialized"));
}

#[test]
fn init_stealth_adds_gitignore_entry() {
    let tmp = TempDir::new().unwrap();

    td().args(["init", "--stealth"])
        .current_dir(&tmp)
        .assert()
        .success();

    let gitignore = std::fs::read_to_string(tmp.path().join(".gitignore")).unwrap();
    assert!(gitignore.contains(".td/"));
}

#[test]
fn init_json_outputs_success() {
    let tmp = TempDir::new().unwrap();

    td().args(["--json", "init"])
        .current_dir(&tmp)
        .assert()
        .success()
        .stdout(predicate::str::contains(r#"{"success":true}"#));
}
