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

#[test]
fn dep_add_creates_blocker() {
    let tmp = init_tmp();
    let a = create_task(&tmp, "Blocked task");
    let b = create_task(&tmp, "Blocker");

    td().args(["dep", "add", &a, &b])
        .current_dir(&tmp)
        .assert()
        .success()
        .stdout(predicate::str::contains("blocked by"));

    let t = get_task_json(&tmp, &a);
    let blockers = t["blockers"].as_array().unwrap();
    assert!(blockers.contains(&serde_json::Value::String(b)));
}

#[test]
fn dep_rm_removes_blocker() {
    let tmp = init_tmp();
    let a = create_task(&tmp, "Was blocked");
    let b = create_task(&tmp, "Was blocker");

    td().args(["dep", "add", &a, &b])
        .current_dir(&tmp)
        .assert()
        .success();
    td().args(["dep", "rm", &a, &b])
        .current_dir(&tmp)
        .assert()
        .success();

    let t = get_task_json(&tmp, &a);
    let blockers = t["blockers"].as_array().unwrap();
    assert!(blockers.is_empty());
}

#[test]
fn dep_tree_shows_children() {
    let tmp = init_tmp();
    let parent = create_task(&tmp, "Parent");

    td().args(["create", "Child one", "--parent", &parent])
        .current_dir(&tmp)
        .assert()
        .success();
    td().args(["create", "Child two", "--parent", &parent])
        .current_dir(&tmp)
        .assert()
        .success();

    td().args(["dep", "tree", &parent])
        .current_dir(&tmp)
        .assert()
        .success()
        .stdout(predicate::str::contains(&parent))
        .stdout(predicate::str::contains(".1"))
        .stdout(predicate::str::contains(".2"));
}
