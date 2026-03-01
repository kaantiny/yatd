use assert_cmd::Command;
use predicates::prelude::*;
use tempfile::TempDir;

fn td(home: &TempDir) -> Command {
    let mut cmd = Command::cargo_bin("td").unwrap();
    cmd.env("HOME", home.path());
    cmd
}

#[test]
fn init_creates_project_snapshot_and_binding() {
    let tmp = TempDir::new().unwrap();

    td(&tmp)
        .args(["init", "demo"])
        .current_dir(&tmp)
        .assert()
        .success()
        .stderr(predicate::str::contains("initialized project 'demo'"));

    let root = tmp.path().join(".local/share/td");
    assert!(root.join("projects/demo/base.loro").is_file());
    assert!(root.join("projects/demo/changes").is_dir());

    let bindings_path = root.join("bindings.json");
    assert!(bindings_path.is_file());
    let bindings: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(bindings_path).unwrap()).unwrap();
    let canonical_cwd = std::fs::canonicalize(tmp.path()).unwrap();
    assert_eq!(
        bindings["bindings"][canonical_cwd.to_string_lossy().as_ref()]
            .as_str()
            .unwrap(),
        "demo"
    );
}

#[test]
fn init_fails_when_project_already_exists() {
    let tmp = TempDir::new().unwrap();

    td(&tmp)
        .args(["init", "demo"])
        .current_dir(&tmp)
        .assert()
        .success();

    td(&tmp)
        .args(["init", "demo"])
        .current_dir(&tmp)
        .assert()
        .failure()
        .stderr(predicate::str::contains("already exists"));
}

#[test]
fn use_binds_another_directory_to_existing_project() {
    let tmp = TempDir::new().unwrap();
    let other = tmp.path().join("other");
    std::fs::create_dir_all(&other).unwrap();

    td(&tmp)
        .args(["init", "demo"])
        .current_dir(&tmp)
        .assert()
        .success();

    td(&tmp)
        .args(["create", "Created from original binding"])
        .current_dir(&tmp)
        .assert()
        .success();

    td(&tmp)
        .args(["use", "demo"])
        .current_dir(&other)
        .assert()
        .success();

    td(&tmp)
        .args(["list"])
        .current_dir(&other)
        .assert()
        .success()
        .stdout(predicate::str::contains("Created from original binding"));
}

#[test]
fn init_json_outputs_success() {
    let tmp = TempDir::new().unwrap();

    td(&tmp)
        .args(["--json", "init", "demo"])
        .current_dir(&tmp)
        .assert()
        .success()
        .stdout(predicate::str::contains(r#""success":true"#))
        .stdout(predicate::str::contains(r#""project":"demo""#));
}

#[test]
fn projects_lists_all_initialized_projects() {
    let tmp = TempDir::new().unwrap();
    let api_dir = tmp.path().join("api");
    let web_dir = tmp.path().join("web");
    std::fs::create_dir_all(&api_dir).unwrap();
    std::fs::create_dir_all(&web_dir).unwrap();

    td(&tmp)
        .args(["init", "api"])
        .current_dir(&api_dir)
        .assert()
        .success();

    td(&tmp)
        .args(["init", "web"])
        .current_dir(&web_dir)
        .assert()
        .success();

    td(&tmp)
        .args(["projects"])
        .current_dir(&api_dir)
        .assert()
        .success()
        .stdout(predicate::str::contains("api"))
        .stdout(predicate::str::contains("web"));
}
