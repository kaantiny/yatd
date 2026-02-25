use assert_cmd::Command;
use predicates::prelude::*;
use tempfile::TempDir;

fn td() -> Command {
    Command::cargo_bin("td").unwrap()
}

fn init_tmp() -> TempDir {
    let tmp = TempDir::new().unwrap();
    td().arg("init").current_dir(&tmp).assert().success();
    tmp
}

/// Create a task and return its JSON id.
fn create_task(dir: &TempDir, title: &str) -> String {
    let out = td()
        .args(["--json", "create", title])
        .current_dir(dir)
        .output()
        .unwrap();
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    v["id"].as_str().unwrap().to_string()
}

// ── list ─────────────────────────────────────────────────────────────

#[test]
fn list_shows_created_tasks() {
    let tmp = init_tmp();
    create_task(&tmp, "Alpha");
    create_task(&tmp, "Bravo");

    td().arg("list")
        .current_dir(&tmp)
        .assert()
        .success()
        .stdout(predicate::str::contains("Alpha"))
        .stdout(predicate::str::contains("Bravo"));
}

#[test]
fn list_json_returns_array() {
    let tmp = init_tmp();
    create_task(&tmp, "One");

    let out = td()
        .args(["--json", "list"])
        .current_dir(&tmp)
        .output()
        .unwrap();
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert!(v.is_array(), "expected JSON array, got: {v}");
    assert_eq!(v.as_array().unwrap().len(), 1);
    assert_eq!(v[0]["title"].as_str().unwrap(), "One");
}

#[test]
fn list_filter_by_status() {
    let tmp = init_tmp();
    create_task(&tmp, "Open task");

    // No closed tasks yet.
    let out = td()
        .args(["--json", "list", "-s", "closed"])
        .current_dir(&tmp)
        .output()
        .unwrap();
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(v.as_array().unwrap().len(), 0);
}

#[test]
fn list_filter_by_priority() {
    let tmp = init_tmp();

    td().args(["create", "Low prio", "-p", "3"])
        .current_dir(&tmp)
        .assert()
        .success();
    td().args(["create", "High prio", "-p", "1"])
        .current_dir(&tmp)
        .assert()
        .success();

    let out = td()
        .args(["--json", "list", "-p", "1"])
        .current_dir(&tmp)
        .output()
        .unwrap();
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    let tasks = v.as_array().unwrap();
    assert_eq!(tasks.len(), 1);
    assert_eq!(tasks[0]["title"].as_str().unwrap(), "High prio");
}

#[test]
fn list_filter_by_label() {
    let tmp = init_tmp();

    td().args(["create", "Tagged", "-l", "urgent"])
        .current_dir(&tmp)
        .assert()
        .success();
    td().args(["create", "Untagged"])
        .current_dir(&tmp)
        .assert()
        .success();

    let out = td()
        .args(["--json", "list", "-l", "urgent"])
        .current_dir(&tmp)
        .output()
        .unwrap();
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    let tasks = v.as_array().unwrap();
    assert_eq!(tasks.len(), 1);
    assert_eq!(tasks[0]["title"].as_str().unwrap(), "Tagged");
}

// ── show ─────────────────────────────────────────────────────────────

#[test]
fn show_displays_task() {
    let tmp = init_tmp();
    let id = create_task(&tmp, "Details here");

    td().args(["show", &id])
        .current_dir(&tmp)
        .assert()
        .success()
        .stdout(predicate::str::contains("Details here"))
        .stdout(predicate::str::contains(&id));
}

#[test]
fn show_json_includes_labels_and_blockers() {
    let tmp = init_tmp();

    td().args(["create", "With labels", "-l", "bug,ui"])
        .current_dir(&tmp)
        .assert()
        .success();

    // Get the id via list.
    let out = td()
        .args(["--json", "list"])
        .current_dir(&tmp)
        .output()
        .unwrap();
    let list: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    let id = list[0]["id"].as_str().unwrap();

    let out = td()
        .args(["--json", "show", id])
        .current_dir(&tmp)
        .output()
        .unwrap();
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(v["title"].as_str().unwrap(), "With labels");

    let labels = v["labels"].as_array().unwrap();
    assert!(labels.contains(&serde_json::Value::String("bug".into())));
    assert!(labels.contains(&serde_json::Value::String("ui".into())));

    // Blockers should be present (even if empty).
    assert!(v["blockers"].is_array());
}

#[test]
fn show_nonexistent_task_fails() {
    let tmp = init_tmp();

    td().args(["show", "td-nope"])
        .current_dir(&tmp)
        .assert()
        .failure()
        .stderr(predicate::str::contains("not found"));
}
