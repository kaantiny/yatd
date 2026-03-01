use assert_cmd::Command;
use predicates::prelude::*;
use tempfile::TempDir;

fn td(home: &TempDir) -> Command {
    let mut cmd = Command::cargo_bin("td").unwrap();
    cmd.env("HOME", home.path());
    cmd
}

/// Initialise a temp directory and return it.
fn init_tmp() -> TempDir {
    let tmp = TempDir::new().unwrap();
    td(&tmp)
        .args(["init", "main"])
        .current_dir(&tmp)
        .assert()
        .success();
    tmp
}

#[test]
fn create_prints_id_and_title() {
    let tmp = init_tmp();

    td(&tmp)
        .args(["create", "My first task"])
        .current_dir(&tmp)
        .assert()
        .success()
        .stdout(predicate::str::contains("My first task"));
}

#[test]
fn create_json_returns_task_object() {
    let tmp = init_tmp();

    td(&tmp)
        .args(["--json", "create", "Buy milk"])
        .current_dir(&tmp)
        .assert()
        .success()
        .stdout(predicate::str::contains(r#""title":"Buy milk"#))
        .stdout(predicate::str::contains(r#""status":"open"#))
        .stdout(predicate::str::contains(r#""priority":"medium""#));
}

#[test]
fn create_with_priority_and_type() {
    let tmp = init_tmp();

    td(&tmp)
        .args(["--json", "create", "Urgent bug", "-p", "high", "-t", "bug"])
        .current_dir(&tmp)
        .assert()
        .success()
        .stdout(predicate::str::contains(r#""priority":"high""#))
        .stdout(predicate::str::contains(r#""type":"bug"#));
}

#[test]
fn create_with_description() {
    let tmp = init_tmp();

    td(&tmp)
        .args([
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

    td(&tmp)
        .args(["--json", "create", "Labelled task", "-l", "frontend,urgent"])
        .current_dir(&tmp)
        .assert()
        .success();

    let out = td(&tmp)
        .args(["--json", "list", "-l", "frontend"])
        .current_dir(&tmp)
        .output()
        .unwrap();
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(v.as_array().unwrap().len(), 1);

    let task = &v[0];
    let labels = task["labels"].as_array().unwrap();
    assert!(labels.contains(&serde_json::Value::String("frontend".to_string())));
    assert!(labels.contains(&serde_json::Value::String("urgent".to_string())));
}

#[test]
fn create_requires_title() {
    let tmp = init_tmp();

    td(&tmp)
        .arg("create")
        .current_dir(&tmp)
        .assert()
        .failure()
        .stderr(predicate::str::contains("title required"));
}

#[test]
fn create_subtask_under_parent() {
    let tmp = init_tmp();

    // Create parent, extract its id.
    let parent_out = td(&tmp)
        .args(["--json", "create", "Parent task"])
        .current_dir(&tmp)
        .output()
        .unwrap();
    let parent: serde_json::Value = serde_json::from_slice(&parent_out.stdout).unwrap();
    let parent_id = parent["id"].as_str().unwrap();

    // Create child under parent.
    let child_out = td(&tmp)
        .args(["--json", "create", "Child task", "--parent", parent_id])
        .current_dir(&tmp)
        .output()
        .unwrap();
    let child: serde_json::Value = serde_json::from_slice(&child_out.stdout).unwrap();
    let child_id = child["id"].as_str().unwrap();

    // Child id is its own ULID; relationship is represented by the parent field.
    assert_ne!(child_id, parent_id);
    assert_eq!(child["parent"].as_str().unwrap(), parent_id);
}

#[test]
fn create_with_effort() {
    let tmp = init_tmp();

    let out = td(&tmp)
        .args(["--json", "create", "Hard task", "-e", "high"])
        .current_dir(&tmp)
        .output()
        .unwrap();
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(v["effort"].as_str().unwrap(), "high");
}

#[test]
fn create_with_priority_label() {
    let tmp = init_tmp();

    let out = td(&tmp)
        .args(["--json", "create", "Low prio", "-p", "low"])
        .current_dir(&tmp)
        .output()
        .unwrap();
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(v["priority"].as_str().unwrap(), "low");
}

#[test]
fn create_rejects_invalid_priority() {
    let tmp = init_tmp();

    td(&tmp)
        .args(["create", "Bad", "-p", "urgent"])
        .current_dir(&tmp)
        .assert()
        .failure()
        .stderr(predicates::prelude::predicate::str::contains(
            "invalid priority",
        ));
}

#[test]
fn create_rejects_invalid_effort() {
    let tmp = init_tmp();

    td(&tmp)
        .args(["create", "Bad", "-e", "huge"])
        .current_dir(&tmp)
        .assert()
        .failure()
        .stderr(predicates::prelude::predicate::str::contains(
            "invalid effort",
        ));
}
