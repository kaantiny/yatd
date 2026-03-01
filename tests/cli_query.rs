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

// ── search ───────────────────────────────────────────────────────────

#[test]
fn search_matches_title() {
    let tmp = init_tmp();
    create_task(&tmp, "Fix login page");
    create_task(&tmp, "Update docs");

    td(&tmp)
        .args(["search", "login"])
        .current_dir(&tmp)
        .assert()
        .success()
        .stdout(predicate::str::contains("Fix login page"));
}

#[test]
fn search_matches_description() {
    let tmp = init_tmp();

    td(&tmp)
        .args(["create", "Vague title", "-d", "The frobnicator is broken"])
        .current_dir(&tmp)
        .assert()
        .success();

    td(&tmp)
        .args(["search", "frobnicator"])
        .current_dir(&tmp)
        .assert()
        .success()
        .stdout(predicate::str::contains("Vague title"));
}

#[test]
fn search_json_returns_array() {
    let tmp = init_tmp();
    create_task(&tmp, "Needle in haystack");

    let out = td(&tmp)
        .args(["--json", "search", "Needle"])
        .current_dir(&tmp)
        .output()
        .unwrap();
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert!(v.is_array());
    assert_eq!(v[0]["title"].as_str().unwrap(), "Needle in haystack");
}

// ── ready ────────────────────────────────────────────────────────────

#[test]
fn ready_excludes_blocked_tasks() {
    let tmp = init_tmp();
    let _a = create_task(&tmp, "Ready task");
    let b = create_task(&tmp, "Blocked task");
    let c = create_task(&tmp, "Blocker task");

    td(&tmp)
        .args(["dep", "add", &b, &c])
        .current_dir(&tmp)
        .assert()
        .success();

    let out = td(&tmp)
        .args(["--json", "ready"])
        .current_dir(&tmp)
        .output()
        .unwrap();
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    let titles: Vec<&str> = v
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["title"].as_str().unwrap())
        .collect();

    assert!(titles.contains(&"Ready task"));
    assert!(titles.contains(&"Blocker task"));
    assert!(!titles.contains(&"Blocked task"));

    // Close the blocker — now the blocked task should become ready.
    td(&tmp)
        .args(["done", &c])
        .current_dir(&tmp)
        .assert()
        .success();

    let out = td(&tmp)
        .args(["--json", "ready"])
        .current_dir(&tmp)
        .output()
        .unwrap();
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    let titles: Vec<&str> = v
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["title"].as_str().unwrap())
        .collect();
    assert!(titles.contains(&"Blocked task"));
    // a is still ready
    assert!(titles.contains(&"Ready task"));
}

// ── stats ────────────────────────────────────────────────────────────

#[test]
fn stats_counts_tasks() {
    let tmp = init_tmp();
    let id = create_task(&tmp, "Open one");
    create_task(&tmp, "Open two");
    td(&tmp)
        .args(["done", &id])
        .current_dir(&tmp)
        .assert()
        .success();

    let out = td(&tmp).args(["stats"]).current_dir(&tmp).output().unwrap();
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(v["total"].as_i64().unwrap(), 2);
    assert_eq!(v["open"].as_i64().unwrap(), 1);
    assert_eq!(v["closed"].as_i64().unwrap(), 1);
}

// ── compact ──────────────────────────────────────────────────────────

#[test]
fn compact_succeeds() {
    let tmp = init_tmp();
    create_task(&tmp, "Anything");

    td(&tmp)
        .arg("compact")
        .current_dir(&tmp)
        .assert()
        .success()
        .stderr(predicate::str::contains("writing compacted snapshot"));
}
