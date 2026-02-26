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
fn rm_deletes_task() {
    let tmp = init_tmp();
    let id = create_task(&tmp, "Delete me");

    td().args(["rm", &id])
        .current_dir(&tmp)
        .assert()
        .success()
        .stdout(predicate::str::contains("deleted"));

    td().args(["show", &id])
        .current_dir(&tmp)
        .assert()
        .failure()
        .stderr(predicate::str::contains("not found"));
}

#[test]
fn rm_deletes_multiple_ids() {
    let tmp = init_tmp();
    let id1 = create_task(&tmp, "First");
    let id2 = create_task(&tmp, "Second");

    td().args(["rm", &id1, &id2])
        .current_dir(&tmp)
        .assert()
        .success();

    td().args(["show", &id1])
        .current_dir(&tmp)
        .assert()
        .failure();
    td().args(["show", &id2])
        .current_dir(&tmp)
        .assert()
        .failure();
}

#[test]
fn rm_requires_recursive_for_parent_task() {
    let tmp = init_tmp();
    let parent = create_task(&tmp, "Parent");
    td().args(["create", "Child", "--parent", &parent])
        .current_dir(&tmp)
        .assert()
        .success();

    td().args(["rm", &parent])
        .current_dir(&tmp)
        .assert()
        .failure()
        .stderr(predicate::str::contains("use --recursive"));
}

#[test]
fn rm_recursive_deletes_subtree() {
    let tmp = init_tmp();
    let parent = create_task(&tmp, "Parent");

    td().args(["create", "Child", "--parent", &parent])
        .current_dir(&tmp)
        .assert()
        .success();
    let child_id = format!("{parent}.1");

    td().args(["create", "Grandchild", "--parent", &child_id])
        .current_dir(&tmp)
        .assert()
        .success();
    let grandchild_id = format!("{child_id}.1");

    td().args(["rm", "--recursive", &parent])
        .current_dir(&tmp)
        .assert()
        .success();

    td().args(["show", &parent])
        .current_dir(&tmp)
        .assert()
        .failure();
    td().args(["show", &child_id])
        .current_dir(&tmp)
        .assert()
        .failure();
    td().args(["show", &grandchild_id])
        .current_dir(&tmp)
        .assert()
        .failure();
}

#[test]
fn rm_detaches_dependents_and_warns() {
    let tmp = init_tmp();
    let dependent = create_task(&tmp, "Dependent");
    let blocker = create_task(&tmp, "Blocker");

    td().args(["dep", "add", &dependent, &blocker])
        .current_dir(&tmp)
        .assert()
        .success();

    td().args(["rm", &blocker])
        .current_dir(&tmp)
        .assert()
        .success()
        .stderr(predicate::str::contains("warning"))
        .stderr(predicate::str::contains(&dependent));

    let dependent_task = get_task_json(&tmp, &dependent);
    let blockers = dependent_task["blockers"].as_array().unwrap();
    assert!(blockers.is_empty());
}

#[test]
fn rm_force_suppresses_unblocked_warning() {
    let tmp = init_tmp();
    let dependent = create_task(&tmp, "Dependent");
    let blocker = create_task(&tmp, "Blocker");

    td().args(["dep", "add", &dependent, &blocker])
        .current_dir(&tmp)
        .assert()
        .success();

    td().args(["rm", "--force", &blocker])
        .current_dir(&tmp)
        .assert()
        .success()
        .stderr(predicate::str::is_empty());
}

#[test]
fn rm_json_includes_deleted_and_unblocked_ids() {
    let tmp = init_tmp();
    let dependent = create_task(&tmp, "Dependent");
    let blocker = create_task(&tmp, "Blocker");

    td().args(["dep", "add", &dependent, &blocker])
        .current_dir(&tmp)
        .assert()
        .success();

    let out = td()
        .args(["--json", "rm", &blocker])
        .current_dir(&tmp)
        .output()
        .unwrap();
    assert!(out.status.success());

    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    let requested = v["requested_ids"].as_array().unwrap();
    let deleted = v["deleted_ids"].as_array().unwrap();
    let unblocked = v["unblocked_ids"].as_array().unwrap();

    assert_eq!(requested, &vec![serde_json::Value::String(blocker.clone())]);
    assert_eq!(deleted, &vec![serde_json::Value::String(blocker)]);
    assert_eq!(unblocked, &vec![serde_json::Value::String(dependent)]);
}
