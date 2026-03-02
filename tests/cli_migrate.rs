use assert_cmd::cargo::cargo_bin_cmd;
use predicates::prelude::*;
use tempfile::TempDir;

fn td(home: &TempDir) -> assert_cmd::Command {
    let mut cmd = cargo_bin_cmd!("td");
    cmd.env("HOME", home.path());
    cmd
}

#[test]
fn init_sets_loro_schema_version_in_meta() {
    let tmp = TempDir::new().unwrap();

    td(&tmp)
        .args(["project", "init", "demo"])
        .current_dir(&tmp)
        .assert()
        .success();

    let root = tmp.path().join(".local/share/td");
    let store = yatd::db::Store::open(&root, "demo").unwrap();
    assert_eq!(store.schema_version().unwrap(), 1);
}

#[test]
fn corrupted_delta_file_is_tolerated_on_open() {
    let tmp = TempDir::new().unwrap();

    td(&tmp)
        .args(["project", "init", "demo"])
        .current_dir(&tmp)
        .assert()
        .success();

    td(&tmp)
        .args(["create", "kept task"])
        .current_dir(&tmp)
        .assert()
        .success();

    let corrupted = tmp
        .path()
        .join(".local/share/td/projects/demo/changes")
        .join("01ARZ3NDEKTSV4RRFFQ69G5FAV.loro");
    std::fs::write(corrupted, b"not-a-valid-loro-delta").unwrap();

    td(&tmp)
        .args(["list"])
        .current_dir(&tmp)
        .assert()
        .success()
        .stdout(predicate::str::contains("kept task"))
        .stderr(predicate::str::contains("skipping unreadable delta"));
}

#[test]
fn project_env_overrides_directory_binding() {
    let tmp = TempDir::new().unwrap();
    let alpha_dir = tmp.path().join("alpha");
    let beta_dir = tmp.path().join("beta");
    std::fs::create_dir_all(&alpha_dir).unwrap();
    std::fs::create_dir_all(&beta_dir).unwrap();

    td(&tmp)
        .args(["project", "init", "alpha"])
        .current_dir(&alpha_dir)
        .assert()
        .success();
    td(&tmp)
        .args(["create", "alpha task"])
        .current_dir(&alpha_dir)
        .assert()
        .success();

    td(&tmp)
        .args(["project", "init", "beta"])
        .current_dir(&beta_dir)
        .assert()
        .success();
    td(&tmp)
        .args(["create", "beta task"])
        .current_dir(&beta_dir)
        .assert()
        .success();

    let out = td(&tmp)
        .args(["--json", "list"])
        .env("TD_PROJECT", "beta")
        .current_dir(&alpha_dir)
        .output()
        .unwrap();
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();

    let titles: Vec<&str> = v
        .as_array()
        .unwrap()
        .iter()
        .map(|task| task["title"].as_str().unwrap())
        .collect();

    assert!(titles.contains(&"beta task"));
    assert!(!titles.contains(&"alpha task"));
}

#[test]
fn legacy_local_sqlite_artifacts_do_not_affect_commands() {
    let tmp = TempDir::new().unwrap();

    td(&tmp)
        .args(["project", "init", "demo"])
        .current_dir(&tmp)
        .assert()
        .success();

    let legacy_dir = tmp.path().join(".td");
    std::fs::create_dir_all(&legacy_dir).unwrap();
    std::fs::write(legacy_dir.join("tasks.db"), b"legacy-sqlite-placeholder").unwrap();

    td(&tmp)
        .args(["create", "new storage path works"])
        .current_dir(&tmp)
        .assert()
        .success();

    td(&tmp)
        .args(["list"])
        .current_dir(&tmp)
        .assert()
        .success()
        .stdout(predicate::str::contains("new storage path works"));
}
