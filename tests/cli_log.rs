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

#[test]
fn log_human_reports_task_id() {
    let tmp = init_tmp();
    let id = create_task(&tmp, "Write docs");

    td().args(["log", &id, "Drafted command docs"])
        .current_dir(&tmp)
        .assert()
        .success()
        .stdout(predicate::str::contains(format!("logged to {id}")));
}

#[test]
fn log_json_emits_created_log_entry() {
    let tmp = init_tmp();
    let id = create_task(&tmp, "Investigate timeout");

    let out = td()
        .args(["--json", "log", &id, "Collected stack traces"])
        .current_dir(&tmp)
        .output()
        .unwrap();
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();

    assert!(v["id"].is_i64());
    assert_eq!(v["task_id"].as_str().unwrap(), id);
    assert_eq!(v["body"].as_str().unwrap(), "Collected stack traces");
    assert!(v["timestamp"].as_str().unwrap().ends_with('Z'));
}

#[test]
fn log_nonexistent_task_fails() {
    let tmp = init_tmp();

    td().args(["log", "td-nope", "No task"])
        .current_dir(&tmp)
        .assert()
        .failure()
        .stderr(predicate::str::contains("task td-nope not found"));
}

#[test]
fn show_human_displays_logs_chronologically() {
    let tmp = init_tmp();
    let id = create_task(&tmp, "Investigate auth issue");

    td().args(["log", &id, "First note"])
        .current_dir(&tmp)
        .assert()
        .success();
    td().args(["log", &id, "Second note"])
        .current_dir(&tmp)
        .assert()
        .success();

    let out = td().args(["show", &id]).current_dir(&tmp).output().unwrap();
    let stdout = String::from_utf8(out.stdout).unwrap();
    let first = stdout.find("First note").unwrap();
    let second = stdout.find("Second note").unwrap();

    assert!(stdout.contains("--- log ---"));
    assert!(first < second, "expected logs in insertion order: {stdout}");
}

#[test]
fn show_json_includes_logs_array() {
    let tmp = init_tmp();
    let id = create_task(&tmp, "Implement parser");

    td().args(["log", &id, "Mapped grammar rules"])
        .current_dir(&tmp)
        .assert()
        .success();

    let out = td()
        .args(["--json", "show", &id])
        .current_dir(&tmp)
        .output()
        .unwrap();
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();

    let logs = v["logs"].as_array().unwrap();
    assert_eq!(logs.len(), 1);
    assert_eq!(logs[0]["body"].as_str().unwrap(), "Mapped grammar rules");
}

#[test]
fn multiple_log_entries_are_ordered() {
    let tmp = init_tmp();
    let id = create_task(&tmp, "Refactor planner");

    for msg in ["step one", "step two", "step three"] {
        td().args(["log", &id, msg])
            .current_dir(&tmp)
            .assert()
            .success();
    }

    let out = td()
        .args(["--json", "show", &id])
        .current_dir(&tmp)
        .output()
        .unwrap();
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    let logs = v["logs"].as_array().unwrap();

    assert_eq!(logs.len(), 3);
    assert_eq!(logs[0]["body"].as_str().unwrap(), "step one");
    assert_eq!(logs[1]["body"].as_str().unwrap(), "step two");
    assert_eq!(logs[2]["body"].as_str().unwrap(), "step three");
}

#[test]
fn export_import_round_trips_logs() {
    let tmp = init_tmp();
    let id = create_task(&tmp, "Port backend");
    td().args(["log", &id, "Measured baseline"])
        .current_dir(&tmp)
        .assert()
        .success();
    td().args(["log", &id, "Applied optimization"])
        .current_dir(&tmp)
        .assert()
        .success();

    let export_out = td().arg("export").current_dir(&tmp).output().unwrap();
    let exported = String::from_utf8(export_out.stdout).unwrap();
    let export_file = tmp.path().join("logs.jsonl");
    std::fs::write(&export_file, &exported).unwrap();

    let tmp2 = TempDir::new().unwrap();
    td().arg("init").current_dir(&tmp2).assert().success();
    td().args(["import", export_file.to_str().unwrap()])
        .current_dir(&tmp2)
        .assert()
        .success();

    let out = td()
        .args(["--json", "show", &id])
        .current_dir(&tmp2)
        .output()
        .unwrap();
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    let logs = v["logs"].as_array().unwrap();

    assert_eq!(logs.len(), 2);
    assert_eq!(logs[0]["body"].as_str().unwrap(), "Measured baseline");
    assert_eq!(logs[1]["body"].as_str().unwrap(), "Applied optimization");
}

#[test]
fn list_json_does_not_include_logs() {
    let tmp = init_tmp();
    let id = create_task(&tmp, "Keep list lean");
    td().args(["log", &id, "This should not surface in list"])
        .current_dir(&tmp)
        .assert()
        .success();

    let out = td()
        .args(["--json", "list"])
        .current_dir(&tmp)
        .output()
        .unwrap();
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();

    assert!(v[0].get("logs").is_none());
}
