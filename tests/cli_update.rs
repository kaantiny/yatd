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

fn create_task(dir: &TempDir, title: &str) -> String {
    let out = td()
        .args(["--json", "create", title])
        .current_dir(dir)
        .output()
        .unwrap();
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    v["id"].as_str().unwrap().to_string()
}

fn get_task_json(dir: &TempDir, id: &str) -> serde_json::Value {
    let out = td()
        .args(["--json", "show", id])
        .current_dir(dir)
        .output()
        .unwrap();
    serde_json::from_slice(&out.stdout).unwrap()
}

// ── update ───────────────────────────────────────────────────────────

#[test]
fn update_changes_status() {
    let tmp = init_tmp();
    let id = create_task(&tmp, "In progress");

    td().args(["update", &id, "-s", "in_progress"])
        .current_dir(&tmp)
        .assert()
        .success()
        .stdout(predicate::str::contains("updated"));

    let t = get_task_json(&tmp, &id);
    assert_eq!(t["status"].as_str().unwrap(), "in_progress");
}

#[test]
fn update_changes_priority() {
    let tmp = init_tmp();
    let id = create_task(&tmp, "Reprioritise");

    td().args(["update", &id, "-p", "high"])
        .current_dir(&tmp)
        .assert()
        .success();

    let t = get_task_json(&tmp, &id);
    assert_eq!(t["priority"].as_i64().unwrap(), 1);
}

#[test]
fn update_changes_title() {
    let tmp = init_tmp();
    let id = create_task(&tmp, "Old title");

    td().args(["update", &id, "-t", "New title"])
        .current_dir(&tmp)
        .assert()
        .success();

    let t = get_task_json(&tmp, &id);
    assert_eq!(t["title"].as_str().unwrap(), "New title");
}

#[test]
fn update_changes_description() {
    let tmp = init_tmp();
    let id = create_task(&tmp, "Describe me");

    td().args(["update", &id, "-d", "Now with details"])
        .current_dir(&tmp)
        .assert()
        .success();

    let t = get_task_json(&tmp, &id);
    assert_eq!(t["description"].as_str().unwrap(), "Now with details");
}

#[test]
fn update_json_returns_task() {
    let tmp = init_tmp();
    let id = create_task(&tmp, "JSON update");

    let out = td()
        .args(["--json", "update", &id, "-p", "high"])
        .current_dir(&tmp)
        .output()
        .unwrap();
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(v["priority"].as_i64().unwrap(), 1);
}

// ── done ─────────────────────────────────────────────────────────────

#[test]
fn done_closes_task() {
    let tmp = init_tmp();
    let id = create_task(&tmp, "Close me");

    td().args(["done", &id])
        .current_dir(&tmp)
        .assert()
        .success()
        .stdout(predicate::str::contains("closed"));

    let t = get_task_json(&tmp, &id);
    assert_eq!(t["status"].as_str().unwrap(), "closed");
}

#[test]
fn done_closes_multiple_tasks() {
    let tmp = init_tmp();
    let id1 = create_task(&tmp, "First");
    let id2 = create_task(&tmp, "Second");

    td().args(["done", &id1, &id2])
        .current_dir(&tmp)
        .assert()
        .success();

    assert_eq!(get_task_json(&tmp, &id1)["status"], "closed");
    assert_eq!(get_task_json(&tmp, &id2)["status"], "closed");
}

// ── reopen ───────────────────────────────────────────────────────────

#[test]
fn reopen_reopens_closed_task() {
    let tmp = init_tmp();
    let id = create_task(&tmp, "Reopen me");

    td().args(["done", &id])
        .current_dir(&tmp)
        .assert()
        .success();
    assert_eq!(get_task_json(&tmp, &id)["status"], "closed");

    td().args(["reopen", &id])
        .current_dir(&tmp)
        .assert()
        .success()
        .stdout(predicate::str::contains("reopened"));

    assert_eq!(get_task_json(&tmp, &id)["status"], "open");
}
