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

fn create_task(dir: &TempDir, title: &str, pri: &str, eff: &str) -> String {
    let out = td()
        .args(["--json", "create", title, "-p", pri, "-e", eff])
        .current_dir(dir)
        .output()
        .unwrap();
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    v["id"].as_str().unwrap().to_string()
}

#[test]
fn next_no_open_tasks() {
    let tmp = init_tmp();

    td().arg("next")
        .current_dir(&tmp)
        .assert()
        .success()
        .stdout(predicate::str::contains("No open tasks"));
}

#[test]
fn next_single_task() {
    let tmp = init_tmp();
    let id = create_task(&tmp, "Only task", "high", "low");

    td().arg("next")
        .current_dir(&tmp)
        .assert()
        .success()
        .stdout(predicate::str::contains(&id))
        .stdout(predicate::str::contains("Only task"))
        .stdout(predicate::str::contains("SCORE"));
}

#[test]
fn next_impact_ranks_by_downstream() {
    let tmp = init_tmp();
    // A blocks B. Same priority/effort. A should rank higher (unblocks B).
    let a = create_task(&tmp, "Blocker", "medium", "medium");
    let b = create_task(&tmp, "Blocked", "medium", "medium");

    td().args(["dep", "add", &b, &a])
        .current_dir(&tmp)
        .assert()
        .success();

    let out = td()
        .args(["--json", "next"])
        .current_dir(&tmp)
        .output()
        .unwrap();
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    let results = v.as_array().unwrap();

    // Only A is ready (B is blocked).
    assert_eq!(results.len(), 1);
    assert_eq!(results[0]["id"].as_str().unwrap(), a);
    assert!(results[0]["total_unblocked"].as_u64().unwrap() > 0);
}

#[test]
fn next_effort_mode_prefers_low_effort() {
    let tmp = init_tmp();
    // Both standalone. A is high-effort, B is low-effort. Same priority.
    let a = create_task(&tmp, "Heavy", "medium", "high");
    let b = create_task(&tmp, "Light", "medium", "low");

    let out = td()
        .args(["--json", "next", "--mode", "effort"])
        .current_dir(&tmp)
        .output()
        .unwrap();
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    let results = v.as_array().unwrap();

    assert_eq!(results.len(), 2);
    // B (low effort) should be ranked first.
    assert_eq!(results[0]["id"].as_str().unwrap(), b);
    assert_eq!(results[1]["id"].as_str().unwrap(), a);
}

#[test]
fn next_verbose_shows_equation() {
    let tmp = init_tmp();
    create_task(&tmp, "Task A", "high", "low");

    td().args(["next", "--verbose"])
        .current_dir(&tmp)
        .assert()
        .success()
        .stdout(predicate::str::contains("mode: impact"))
        .stdout(predicate::str::contains("Unblocks:"));
}

#[test]
fn next_verbose_effort_mode_shows_squared() {
    let tmp = init_tmp();
    create_task(&tmp, "Task A", "high", "medium");

    td().args(["next", "--verbose", "--mode", "effort"])
        .current_dir(&tmp)
        .assert()
        .success()
        .stdout(predicate::str::contains("mode: effort"))
        .stdout(predicate::str::contains("\u{00b2}"));
}

#[test]
fn next_limit_truncates() {
    let tmp = init_tmp();
    create_task(&tmp, "A", "high", "low");
    create_task(&tmp, "B", "medium", "medium");
    create_task(&tmp, "C", "low", "high");

    let out = td()
        .args(["--json", "next", "-n", "2"])
        .current_dir(&tmp)
        .output()
        .unwrap();
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(v.as_array().unwrap().len(), 2);
}

#[test]
fn next_json_empty() {
    let tmp = init_tmp();

    let out = td()
        .args(["--json", "next"])
        .current_dir(&tmp)
        .output()
        .unwrap();
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(v.as_array().unwrap().len(), 0);
}

#[test]
fn next_invalid_mode_fails() {
    let tmp = init_tmp();
    create_task(&tmp, "X", "medium", "medium");

    td().args(["next", "--mode", "bogus"])
        .current_dir(&tmp)
        .assert()
        .failure()
        .stderr(predicate::str::contains("invalid mode"));
}

#[test]
fn next_transitive_chain_scores_correctly() {
    let tmp = init_tmp();
    // A blocks B, B blocks C. Only A is ready.
    let a = create_task(&tmp, "Root", "medium", "low");
    let b = create_task(&tmp, "Mid", "high", "medium");
    let c = create_task(&tmp, "Leaf", "low", "high");

    td().args(["dep", "add", &b, &a])
        .current_dir(&tmp)
        .assert()
        .success();
    td().args(["dep", "add", &c, &b])
        .current_dir(&tmp)
        .assert()
        .success();

    let out = td()
        .args(["--json", "next"])
        .current_dir(&tmp)
        .output()
        .unwrap();
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    let results = v.as_array().unwrap();

    assert_eq!(results.len(), 1);
    assert_eq!(results[0]["id"].as_str().unwrap(), a);
    // Downstream: pw(B=high=3) + pw(C=low=1) = 4.0
    assert!((results[0]["downstream_score"].as_f64().unwrap() - 4.0).abs() < f64::EPSILON);
    assert_eq!(results[0]["total_unblocked"].as_u64().unwrap(), 2);
    assert_eq!(results[0]["direct_unblocked"].as_u64().unwrap(), 1);
}

#[test]
fn next_ignores_closed_tasks() {
    let tmp = init_tmp();
    let a = create_task(&tmp, "Open", "high", "low");
    let b = create_task(&tmp, "Closed", "high", "low");

    td().args(["done", &b]).current_dir(&tmp).assert().success();

    let out = td()
        .args(["--json", "next"])
        .current_dir(&tmp)
        .output()
        .unwrap();
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    let results = v.as_array().unwrap();

    assert_eq!(results.len(), 1);
    assert_eq!(results[0]["id"].as_str().unwrap(), a);
}

#[test]
fn next_excludes_parent_with_open_subtasks() {
    let tmp = init_tmp();
    let parent = create_task(&tmp, "Parent task", "high", "low");
    // Create a subtask under the parent.
    let out = td()
        .args([
            "--json",
            "create",
            "Child task",
            "-p",
            "medium",
            "-e",
            "medium",
            "--parent",
            &parent,
        ])
        .current_dir(&tmp)
        .output()
        .unwrap();
    let child: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    let child_id = child["id"].as_str().unwrap().to_string();

    let out = td()
        .args(["--json", "next"])
        .current_dir(&tmp)
        .output()
        .unwrap();
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    let results = v.as_array().unwrap();
    let ids: Vec<&str> = results.iter().map(|r| r["id"].as_str().unwrap()).collect();

    // Parent should be excluded; only the child subtask should appear.
    assert!(!ids.contains(&parent.as_str()), "parent should be excluded");
    assert!(
        ids.contains(&child_id.as_str()),
        "child should be a candidate"
    );
}

#[test]
fn next_includes_parent_when_all_subtasks_closed() {
    let tmp = init_tmp();
    let parent = create_task(&tmp, "Parent task", "high", "low");
    let out = td()
        .args([
            "--json",
            "create",
            "Child task",
            "-p",
            "medium",
            "-e",
            "medium",
            "--parent",
            &parent,
        ])
        .current_dir(&tmp)
        .output()
        .unwrap();
    let child: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    let child_id = child["id"].as_str().unwrap().to_string();

    // Close the subtask.
    td().args(["done", &child_id])
        .current_dir(&tmp)
        .assert()
        .success();

    let out = td()
        .args(["--json", "next"])
        .current_dir(&tmp)
        .output()
        .unwrap();
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    let results = v.as_array().unwrap();
    let ids: Vec<&str> = results.iter().map(|r| r["id"].as_str().unwrap()).collect();

    // Parent should reappear as a candidate once all children are closed.
    assert!(
        ids.contains(&parent.as_str()),
        "parent should be a candidate"
    );
}

#[test]
fn next_nested_parents_excluded_at_each_level() {
    let tmp = init_tmp();
    // grandparent → parent → child (nested subtasks)
    let gp = create_task(&tmp, "Grandparent", "high", "low");
    let out = td()
        .args([
            "--json", "create", "Parent", "-p", "medium", "-e", "medium", "--parent", &gp,
        ])
        .current_dir(&tmp)
        .output()
        .unwrap();
    let p: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    let p_id = p["id"].as_str().unwrap().to_string();

    let out = td()
        .args([
            "--json", "create", "Child", "-p", "low", "-e", "low", "--parent", &p_id,
        ])
        .current_dir(&tmp)
        .output()
        .unwrap();
    let c: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    let c_id = c["id"].as_str().unwrap().to_string();

    let out = td()
        .args(["--json", "next"])
        .current_dir(&tmp)
        .output()
        .unwrap();
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    let results = v.as_array().unwrap();
    let ids: Vec<&str> = results.iter().map(|r| r["id"].as_str().unwrap()).collect();

    // Both grandparent and parent are excluded; only the leaf child appears.
    assert!(
        !ids.contains(&gp.as_str()),
        "grandparent should be excluded"
    );
    assert!(!ids.contains(&p_id.as_str()), "parent should be excluded");
    assert!(
        ids.contains(&c_id.as_str()),
        "leaf child should be a candidate"
    );
}
