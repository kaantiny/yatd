use assert_cmd::cargo::cargo_bin_cmd;
use predicates::prelude::*;
use tempfile::TempDir;

fn td(home: &TempDir) -> assert_cmd::Command {
    let mut cmd = cargo_bin_cmd!("td");
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

/// Create a task and return its JSON id.
fn create_task(dir: &TempDir, title: &str) -> String {
    let out = td(dir)
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

    td(&tmp)
        .arg("list")
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

    let out = td(&tmp)
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
    let out = td(&tmp)
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

    td(&tmp)
        .args(["create", "Low prio", "-p", "low"])
        .current_dir(&tmp)
        .assert()
        .success();
    td(&tmp)
        .args(["create", "High prio", "-p", "high"])
        .current_dir(&tmp)
        .assert()
        .success();

    let out = td(&tmp)
        .args(["--json", "list", "-p", "high"])
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

    td(&tmp)
        .args(["create", "Tagged", "-l", "urgent"])
        .current_dir(&tmp)
        .assert()
        .success();
    td(&tmp)
        .args(["create", "Untagged"])
        .current_dir(&tmp)
        .assert()
        .success();

    let out = td(&tmp)
        .args(["--json", "list", "-l", "urgent"])
        .current_dir(&tmp)
        .output()
        .unwrap();
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    let tasks = v.as_array().unwrap();
    assert_eq!(tasks.len(), 1);
    assert_eq!(tasks[0]["title"].as_str().unwrap(), "Tagged");
}

#[test]
fn list_filter_by_effort() {
    let tmp = init_tmp();

    td(&tmp)
        .args(["create", "Easy", "-e", "low"])
        .current_dir(&tmp)
        .assert()
        .success();
    td(&tmp)
        .args(["create", "Hard", "-e", "high"])
        .current_dir(&tmp)
        .assert()
        .success();

    let out = td(&tmp)
        .args(["--json", "list", "-e", "low"])
        .current_dir(&tmp)
        .output()
        .unwrap();
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    let tasks = v.as_array().unwrap();
    assert_eq!(tasks.len(), 1);
    assert_eq!(tasks[0]["title"].as_str().unwrap(), "Easy");
}

// ── show ─────────────────────────────────────────────────────────────

#[test]
fn show_displays_task() {
    let tmp = init_tmp();
    let id = create_task(&tmp, "Details here");

    td(&tmp)
        .args(["show", &id])
        .current_dir(&tmp)
        .assert()
        .success()
        .stdout(predicate::str::contains("Details here"))
        .stdout(predicate::str::contains(&id[id.len() - 7..]));
}

#[test]
fn show_json_includes_labels_and_blockers() {
    let tmp = init_tmp();

    td(&tmp)
        .args(["create", "With labels", "-l", "bug,ui"])
        .current_dir(&tmp)
        .assert()
        .success();

    // Get the id via list.
    let out = td(&tmp)
        .args(["--json", "list"])
        .current_dir(&tmp)
        .output()
        .unwrap();
    let list: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    let id = list[0]["id"].as_str().unwrap();

    let out = td(&tmp)
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

    td(&tmp)
        .args(["show", "td-nope"])
        .current_dir(&tmp)
        .assert()
        .failure()
        .stderr(predicate::str::contains("not found"));
}

#[test]
fn show_annotates_closed_blockers() {
    let tmp = init_tmp();
    let task = create_task(&tmp, "Blocked task");
    let open_blocker = create_task(&tmp, "Still open");
    let closed_blocker = create_task(&tmp, "Will close");

    td(&tmp)
        .args(["dep", "add", &task, &open_blocker])
        .current_dir(&tmp)
        .assert()
        .success();
    td(&tmp)
        .args(["dep", "add", &task, &closed_blocker])
        .current_dir(&tmp)
        .assert()
        .success();
    td(&tmp)
        .args(["done", &closed_blocker])
        .current_dir(&tmp)
        .assert()
        .success();

    // Plural label, open blocker bare, closed one annotated.
    td(&tmp)
        .args(["show", &task])
        .current_dir(&tmp)
        .assert()
        .success()
        .stdout(predicate::str::contains("blockers"))
        .stdout(predicate::str::contains(
            &open_blocker[open_blocker.len() - 7..],
        ))
        .stdout(predicate::str::contains(&format!(
            "{} [closed]",
            &closed_blocker[closed_blocker.len() - 7..]
        )));
}

#[test]
fn show_all_closed_blockers_prefixed() {
    let tmp = init_tmp();
    let task = create_task(&tmp, "Was blocked");
    let blocker = create_task(&tmp, "Done now");

    td(&tmp)
        .args(["dep", "add", &task, &blocker])
        .current_dir(&tmp)
        .assert()
        .success();
    td(&tmp)
        .args(["done", &blocker])
        .current_dir(&tmp)
        .assert()
        .success();

    // Singular label, [all closed] prefix.
    td(&tmp)
        .args(["show", &task])
        .current_dir(&tmp)
        .assert()
        .success()
        .stdout(predicate::str::contains("blocker"))
        .stdout(predicate::str::contains("[all closed]"))
        .stdout(predicate::str::contains(&blocker[blocker.len() - 7..]));
}

#[test]
fn show_single_open_blocker_singular_label() {
    let tmp = init_tmp();
    let task = create_task(&tmp, "Blocked");
    let blocker = create_task(&tmp, "Blocking");

    td(&tmp)
        .args(["dep", "add", &task, &blocker])
        .current_dir(&tmp)
        .assert()
        .success();

    let out = td(&tmp)
        .args(["show", &task])
        .current_dir(&tmp)
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&out.stdout);

    // Singular "blocker", no "blockers".
    assert!(stdout.contains("blocker"));
    assert!(stdout.contains(&blocker[blocker.len() - 7..]));
    // Should not contain [closed] or [all closed].
    assert!(!stdout.contains("[closed]"));
}
