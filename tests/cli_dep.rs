use assert_cmd::Command;
use predicates::prelude::*;
use tempfile::TempDir;

fn td(home: &TempDir) -> Command {
    let mut cmd = Command::cargo_bin("td").unwrap();
    cmd.env("HOME", home.path());
    cmd
}

fn init_tmp() -> TempDir {
    let tmp = TempDir::new().unwrap();
    td(&tmp)
        .args(["project", "init", "main"])
        .current_dir(&tmp)
        .assert()
        .success();
    tmp
}

fn create_task(dir: &TempDir, title: &str) -> String {
    let out = td(dir)
        .args(["--json", "create", title])
        .current_dir(dir)
        .output()
        .unwrap();
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    v["id"].as_str().unwrap().to_string()
}

fn get_task_json(dir: &TempDir, id: &str) -> serde_json::Value {
    let out = td(dir)
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

    td(&tmp)
        .args(["dep", "add", &a, &b])
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

    td(&tmp)
        .args(["dep", "add", &a, &b])
        .current_dir(&tmp)
        .assert()
        .success();
    td(&tmp)
        .args(["dep", "rm", &a, &b])
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

    let out = td(&tmp)
        .args(["--json", "create", "Subtask one", "--parent", &parent])
        .current_dir(&tmp)
        .output()
        .unwrap();
    let subtask_one: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    let subtask_one_id = subtask_one["id"].as_str().unwrap().to_string();

    let out = td(&tmp)
        .args(["--json", "create", "Subtask two", "--parent", &parent])
        .current_dir(&tmp)
        .output()
        .unwrap();
    let subtask_two: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    let subtask_two_id = subtask_two["id"].as_str().unwrap().to_string();

    td(&tmp)
        .args(["dep", "tree", &parent])
        .current_dir(&tmp)
        .assert()
        .success()
        .stdout(predicate::str::contains(&parent[parent.len() - 7..]))
        .stdout(predicate::str::contains(
            &subtask_one_id[subtask_one_id.len() - 7..],
        ))
        .stdout(predicate::str::contains(
            &subtask_two_id[subtask_two_id.len() - 7..],
        ));
}

#[test]
fn dep_add_rejects_self_cycle() {
    let tmp = init_tmp();
    let a = create_task(&tmp, "Self-referential");

    td(&tmp)
        .args(["dep", "add", &a, &a])
        .current_dir(&tmp)
        .assert()
        .failure()
        .stderr(predicate::str::contains("cycle"));
}

#[test]
fn dep_add_rejects_direct_cycle() {
    let tmp = init_tmp();
    let a = create_task(&tmp, "Task A");
    let b = create_task(&tmp, "Task B");

    // A blocked by B
    td(&tmp)
        .args(["dep", "add", &a, &b])
        .current_dir(&tmp)
        .assert()
        .success();

    // B blocked by A would create A → B → A
    td(&tmp)
        .args(["dep", "add", &b, &a])
        .current_dir(&tmp)
        .assert()
        .failure()
        .stderr(predicate::str::contains("cycle"));
}

#[test]
fn dep_add_rejects_transitive_cycle() {
    let tmp = init_tmp();
    let a = create_task(&tmp, "Task A");
    let b = create_task(&tmp, "Task B");
    let c = create_task(&tmp, "Task C");

    // A blocked by B, B blocked by C
    td(&tmp)
        .args(["dep", "add", &a, &b])
        .current_dir(&tmp)
        .assert()
        .success();
    td(&tmp)
        .args(["dep", "add", &b, &c])
        .current_dir(&tmp)
        .assert()
        .success();

    // C blocked by A would create A → B → C → A
    td(&tmp)
        .args(["dep", "add", &c, &a])
        .current_dir(&tmp)
        .assert()
        .failure()
        .stderr(predicate::str::contains("cycle"));
}

#[test]
fn dep_add_allows_diamond_without_cycle() {
    let tmp = init_tmp();
    let a = create_task(&tmp, "Task A");
    let b = create_task(&tmp, "Task B");
    let c = create_task(&tmp, "Task C");
    let d = create_task(&tmp, "Task D");

    // Diamond: D blocked by B and C, both blocked by A
    td(&tmp)
        .args(["dep", "add", &d, &b])
        .current_dir(&tmp)
        .assert()
        .success();
    td(&tmp)
        .args(["dep", "add", &d, &c])
        .current_dir(&tmp)
        .assert()
        .success();
    td(&tmp)
        .args(["dep", "add", &b, &a])
        .current_dir(&tmp)
        .assert()
        .success();
    td(&tmp)
        .args(["dep", "add", &c, &a])
        .current_dir(&tmp)
        .assert()
        .success();

    // Verify all edges exist — no false cycle detection
    let t = get_task_json(&tmp, &d);
    let blockers = t["blockers"].as_array().unwrap();
    assert_eq!(blockers.len(), 2);
}

#[test]
fn dep_add_rejects_nonexistent_child() {
    let tmp = init_tmp();
    let real = create_task(&tmp, "Real task");

    td(&tmp)
        .args(["dep", "add", "td-ghost", &real])
        .current_dir(&tmp)
        .assert()
        .failure()
        .stderr(predicate::str::contains("task 'ghost' not found"));
}

#[test]
fn dep_add_rejects_nonexistent_parent() {
    let tmp = init_tmp();
    let real = create_task(&tmp, "Real task");

    td(&tmp)
        .args(["dep", "add", &real, "td-phantom"])
        .current_dir(&tmp)
        .assert()
        .failure()
        .stderr(predicate::str::contains("task 'phantom' not found"));
}
