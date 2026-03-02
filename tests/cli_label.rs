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

#[test]
fn label_add_and_list() {
    let tmp = init_tmp();
    let id = create_task(&tmp, "Tag me");

    td(&tmp)
        .args(["label", "add", &id, "important"])
        .current_dir(&tmp)
        .assert()
        .success()
        .stdout(predicate::str::contains("added"));

    td(&tmp)
        .args(["label", "list", &id])
        .current_dir(&tmp)
        .assert()
        .success()
        .stdout(predicate::str::contains("important"));
}

#[test]
fn label_rm_removes_label() {
    let tmp = init_tmp();
    let id = create_task(&tmp, "Untag me");

    td(&tmp)
        .args(["label", "add", &id, "temp"])
        .current_dir(&tmp)
        .assert()
        .success();
    td(&tmp)
        .args(["label", "rm", &id, "temp"])
        .current_dir(&tmp)
        .assert()
        .success();

    td(&tmp)
        .args(["label", "list", &id])
        .current_dir(&tmp)
        .assert()
        .success()
        .stdout(predicate::str::is_empty());
}

#[test]
fn label_list_all_shows_distinct_labels() {
    let tmp = init_tmp();
    let a = create_task(&tmp, "A");
    let b = create_task(&tmp, "B");

    td(&tmp)
        .args(["label", "add", &a, "bug"])
        .current_dir(&tmp)
        .assert()
        .success();
    td(&tmp)
        .args(["label", "add", &b, "bug"])
        .current_dir(&tmp)
        .assert()
        .success();
    td(&tmp)
        .args(["label", "add", &b, "ui"])
        .current_dir(&tmp)
        .assert()
        .success();

    td(&tmp)
        .args(["label", "list-all"])
        .current_dir(&tmp)
        .assert()
        .success()
        .stdout(predicate::str::contains("bug"))
        .stdout(predicate::str::contains("ui"));
}
