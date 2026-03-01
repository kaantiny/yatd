use assert_cmd::Command;
use tempfile::TempDir;

fn td(home: &TempDir) -> Command {
    let mut cmd = Command::cargo_bin("td").unwrap();
    cmd.env("HOME", home.path());
    cmd
}

fn init_tmp() -> TempDir {
    let tmp = TempDir::new().unwrap();
    td(&tmp)
        .args(["init", "main"])
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
fn export_produces_jsonl() {
    let tmp = init_tmp();
    create_task(&tmp, "First");
    create_task(&tmp, "Second");

    let out = td(&tmp).arg("export").current_dir(&tmp).output().unwrap();
    let stdout = String::from_utf8(out.stdout).unwrap();
    let lines: Vec<&str> = stdout.lines().collect();
    assert_eq!(lines.len(), 2, "expected 2 JSONL lines, got: {stdout}");

    // Each line should be valid JSON with an id field.
    for line in &lines {
        let v: serde_json::Value = serde_json::from_str(line).unwrap();
        assert!(v["id"].is_string());
    }
}

#[test]
fn export_includes_labels_and_blockers() {
    let tmp = init_tmp();
    td(&tmp)
        .args(["create", "With labels", "-l", "bug"])
        .current_dir(&tmp)
        .assert()
        .success();

    let out = td(&tmp).arg("export").current_dir(&tmp).output().unwrap();
    let line = String::from_utf8(out.stdout).unwrap();
    let v: serde_json::Value = serde_json::from_str(line.trim()).unwrap();
    assert!(v["labels"].is_array());
    assert!(v["blockers"].is_array());
}

#[test]
fn import_round_trips_with_export() {
    let tmp = init_tmp();
    create_task(&tmp, "Alpha");

    td(&tmp)
        .args(["create", "Bravo", "-l", "important"])
        .current_dir(&tmp)
        .assert()
        .success();

    // Export.
    let export_out = td(&tmp).arg("export").current_dir(&tmp).output().unwrap();
    let exported = String::from_utf8(export_out.stdout).unwrap();

    // Write to a file.
    let export_file = tmp.path().join("backup.jsonl");
    std::fs::write(&export_file, &exported).unwrap();

    // Create a fresh directory, init, import.
    let tmp2 = TempDir::new().unwrap();
    td(&tmp2)
        .args(["init", "mirror"])
        .current_dir(&tmp2)
        .assert()
        .success();

    td(&tmp2)
        .args(["import", export_file.to_str().unwrap()])
        .current_dir(&tmp2)
        .assert()
        .success();

    // Verify tasks exist in the new database.
    let out = td(&tmp2)
        .args(["--json", "list"])
        .current_dir(&tmp2)
        .output()
        .unwrap();
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    let titles: Vec<&str> = v
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["title"].as_str().unwrap())
        .collect();
    assert!(titles.contains(&"Alpha"));
    assert!(titles.contains(&"Bravo"));

    // Verify labels survived.
    let bravo = v
        .as_array()
        .unwrap()
        .iter()
        .find(|t| t["title"] == "Bravo")
        .unwrap();
    let labels = bravo["labels"].as_array().unwrap();
    assert!(labels.contains(&serde_json::Value::String("important".into())));
}

#[test]
fn export_import_preserves_effort() {
    let tmp = init_tmp();

    td(&tmp)
        .args(["create", "High effort", "-e", "high"])
        .current_dir(&tmp)
        .assert()
        .success();

    // Export.
    let out = td(&tmp).arg("export").current_dir(&tmp).output().unwrap();
    let exported = String::from_utf8(out.stdout).unwrap();

    // Verify effort is in the JSONL.
    let v: serde_json::Value = serde_json::from_str(exported.trim()).unwrap();
    assert_eq!(v["effort"].as_str().unwrap(), "high");

    // Round-trip into a fresh database.
    let export_file = tmp.path().join("effort.jsonl");
    std::fs::write(&export_file, &exported).unwrap();

    let tmp2 = TempDir::new().unwrap();
    td(&tmp2)
        .args(["init", "mirror"])
        .current_dir(&tmp2)
        .assert()
        .success();
    td(&tmp2)
        .args(["import", export_file.to_str().unwrap()])
        .current_dir(&tmp2)
        .assert()
        .success();

    let out2 = td(&tmp2)
        .args(["--json", "list"])
        .current_dir(&tmp2)
        .output()
        .unwrap();
    let v2: serde_json::Value = serde_json::from_slice(&out2.stdout).unwrap();
    assert_eq!(v2[0]["effort"].as_str().unwrap(), "high");
}
