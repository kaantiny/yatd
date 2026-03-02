use assert_cmd::cargo::cargo_bin_cmd;
use predicates::prelude::*;
use tempfile::TempDir;

fn td(home: &TempDir) -> assert_cmd::Command {
    let mut cmd = cargo_bin_cmd!("td");
    cmd.env("HOME", home.path());
    cmd
}

// ---------------------------------------------------------------------------
// init
// ---------------------------------------------------------------------------

#[test]
fn init_creates_project_snapshot_and_binding() {
    let tmp = TempDir::new().unwrap();

    td(&tmp)
        .args(["project", "init", "demo"])
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
        .args(["project", "init", "demo"])
        .current_dir(&tmp)
        .assert()
        .success();

    td(&tmp)
        .args(["project", "init", "demo"])
        .current_dir(&tmp)
        .assert()
        .failure()
        .stderr(predicate::str::contains("already exists"));
}

#[test]
fn init_json_outputs_success() {
    let tmp = TempDir::new().unwrap();

    td(&tmp)
        .args(["--json", "project", "init", "demo"])
        .current_dir(&tmp)
        .assert()
        .success()
        .stdout(predicate::str::contains(r#""success":true"#))
        .stdout(predicate::str::contains(r#""project":"demo""#));
}

// ---------------------------------------------------------------------------
// bind
// ---------------------------------------------------------------------------

#[test]
fn bind_links_another_directory_to_existing_project() {
    let tmp = TempDir::new().unwrap();
    let other = tmp.path().join("other");
    std::fs::create_dir_all(&other).unwrap();

    td(&tmp)
        .args(["project", "init", "demo"])
        .current_dir(&tmp)
        .assert()
        .success();

    td(&tmp)
        .args(["create", "Created from original binding"])
        .current_dir(&tmp)
        .assert()
        .success();

    td(&tmp)
        .args(["project", "bind", "demo"])
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

// ---------------------------------------------------------------------------
// list
// ---------------------------------------------------------------------------

#[test]
fn list_shows_all_initialized_projects() {
    let tmp = TempDir::new().unwrap();
    let api_dir = tmp.path().join("api");
    let web_dir = tmp.path().join("web");
    std::fs::create_dir_all(&api_dir).unwrap();
    std::fs::create_dir_all(&web_dir).unwrap();

    td(&tmp)
        .args(["project", "init", "api"])
        .current_dir(&api_dir)
        .assert()
        .success();

    td(&tmp)
        .args(["project", "init", "web"])
        .current_dir(&web_dir)
        .assert()
        .success();

    td(&tmp)
        .args(["project", "list"])
        .current_dir(&api_dir)
        .assert()
        .success()
        .stdout(predicate::str::contains("api"))
        .stdout(predicate::str::contains("web"));
}

// ---------------------------------------------------------------------------
// unbind
// ---------------------------------------------------------------------------

#[test]
fn unbind_removes_binding_and_subsequent_list_fails() {
    let tmp = TempDir::new().unwrap();

    td(&tmp)
        .args(["project", "init", "demo"])
        .current_dir(&tmp)
        .assert()
        .success();

    // Verify the binding works before we remove it.
    td(&tmp).args(["list"]).current_dir(&tmp).assert().success();

    td(&tmp)
        .args(["project", "unbind"])
        .current_dir(&tmp)
        .assert()
        .success();

    // Without an explicit --project flag, list should now fail.
    td(&tmp)
        .args(["list"])
        .current_dir(&tmp)
        .assert()
        .failure()
        .stderr(predicate::str::contains("no project selected"));
}

#[test]
fn unbind_fails_when_no_binding_exists() {
    let tmp = TempDir::new().unwrap();

    td(&tmp)
        .args(["project", "unbind"])
        .current_dir(&tmp)
        .assert()
        .failure()
        .stderr(predicate::str::contains("not bound to any project"));
}

// ---------------------------------------------------------------------------
// delete
// ---------------------------------------------------------------------------

#[test]
fn delete_removes_project_data_and_binding() {
    let tmp = TempDir::new().unwrap();

    td(&tmp)
        .args(["project", "init", "demo"])
        .current_dir(&tmp)
        .assert()
        .success();

    let root = tmp.path().join(".local/share/td");
    assert!(root.join("projects/demo/base.loro").is_file());

    td(&tmp)
        .args(["project", "delete", "demo"])
        .current_dir(&tmp)
        .assert()
        .success();

    // Project directory should be gone.
    assert!(!root.join("projects/demo").exists());

    // Binding should be removed from bindings.json.
    let bindings: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(root.join("bindings.json")).unwrap())
            .unwrap();
    let canonical_cwd = std::fs::canonicalize(tmp.path()).unwrap();
    assert!(
        bindings["bindings"][canonical_cwd.to_string_lossy().as_ref()].is_null(),
        "binding should have been removed after project deletion"
    );
}

#[test]
fn delete_fails_for_nonexistent_project() {
    let tmp = TempDir::new().unwrap();

    // We still need at least one project so the data dir exists, but we
    // delete one that was never created.
    td(&tmp)
        .args(["project", "init", "real"])
        .current_dir(&tmp)
        .assert()
        .success();

    td(&tmp)
        .args(["project", "delete", "nonexistent"])
        .current_dir(&tmp)
        .assert()
        .failure()
        .stderr(predicate::str::contains("project 'nonexistent' not found"));
}

#[test]
fn delete_cleans_up_all_bindings_for_project() {
    let tmp = TempDir::new().unwrap();
    let dir_a = tmp.path().join("a");
    let dir_b = tmp.path().join("b");
    std::fs::create_dir_all(&dir_a).unwrap();
    std::fs::create_dir_all(&dir_b).unwrap();

    // Init in dir_a, bind dir_b to the same project.
    td(&tmp)
        .args(["project", "init", "shared"])
        .current_dir(&dir_a)
        .assert()
        .success();

    td(&tmp)
        .args(["project", "bind", "shared"])
        .current_dir(&dir_b)
        .assert()
        .success();

    // Sanity-check: both dirs should resolve.
    td(&tmp)
        .args(["list"])
        .current_dir(&dir_a)
        .assert()
        .success();
    td(&tmp)
        .args(["list"])
        .current_dir(&dir_b)
        .assert()
        .success();

    // Delete the project entirely.
    td(&tmp)
        .args(["project", "delete", "shared"])
        .current_dir(&dir_a)
        .assert()
        .success();

    // Neither directory should have a binding any more.
    let bindings: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(tmp.path().join(".local/share/td/bindings.json")).unwrap(),
    )
    .unwrap();

    let canonical_a = std::fs::canonicalize(&dir_a).unwrap();
    let canonical_b = std::fs::canonicalize(&dir_b).unwrap();
    assert!(
        bindings["bindings"][canonical_a.to_string_lossy().as_ref()].is_null(),
        "binding for dir_a should have been removed"
    );
    assert!(
        bindings["bindings"][canonical_b.to_string_lossy().as_ref()].is_null(),
        "binding for dir_b should have been removed"
    );
}
