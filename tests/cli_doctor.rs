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

fn create_task(dir: &TempDir, title: &str) -> String {
    let out = td(dir)
        .args(["--json", "create", title])
        .current_dir(dir)
        .output()
        .unwrap();
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    v["id"].as_str().unwrap().to_string()
}

fn import_jsonl(dir: &TempDir, lines: &[&str]) {
    let file = dir.path().join("import.jsonl");
    std::fs::write(&file, lines.join("\n")).unwrap();
    td(dir)
        .args(["import", file.to_str().unwrap()])
        .current_dir(dir)
        .assert()
        .success();
}

fn doctor_json(dir: &TempDir, fix: bool) -> serde_json::Value {
    let mut args = vec!["--json", "doctor"];
    if fix {
        args.push("--fix");
    }
    let out = td(dir).args(&args).current_dir(dir).output().unwrap();
    assert!(
        out.status.success(),
        "doctor failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    serde_json::from_slice(&out.stdout).unwrap()
}

// Valid 26-char ULIDs that don't correspond to any real task.
// Crockford Base32 excludes I, L, O, U — all chars below are valid.
const GHOST1: &str = "00000000000000000000DEAD01";
const GHOST2: &str = "00000000000000000000DEAD02";
const GHOST3: &str = "00000000000000000000DEAD03";
const GHOST4: &str = "00000000000000000000DEAD04";
const GHOST5: &str = "00000000000000000000DEAD05";
const GHOST6: &str = "00000000000000000000DEAD06";

// Fixed ULIDs for tasks we create via import.
const TASK01: &str = "01HQ0000000000000000000001";
const TASK02: &str = "01HQ0000000000000000000002";
const TASK03: &str = "01HQ0000000000000000000003";
const TASK04: &str = "01HQ0000000000000000000004";
const TASK05: &str = "01HQ0000000000000000000005";
const TASK06: &str = "01HQ0000000000000000000006";
const TASK07: &str = "01HQ0000000000000000000007";
const TASK08: &str = "01HQ0000000000000000000008";
const TASK0A: &str = "01HQ000000000000000000000A";
const TASK0B: &str = "01HQ000000000000000000000B";
const TASK0C: &str = "01HQ000000000000000000000C";
const TASK10: &str = "01HQ0000000000000000000010";
const TASK11: &str = "01HQ0000000000000000000011";
const TASK12: &str = "01HQ0000000000000000000012";
const TASK13: &str = "01HQ0000000000000000000013";
const TASK20: &str = "01HQ0000000000000000000020";
const TASK21: &str = "01HQ0000000000000000000021";
const TASK22: &str = "01HQ0000000000000000000022";
const TASK30: &str = "01HQ0000000000000000000030";

// --- Clean project ---

#[test]
fn doctor_clean_project_reports_no_issues() {
    let tmp = init_tmp();
    create_task(&tmp, "Healthy task");

    td(&tmp)
        .args(["doctor"])
        .current_dir(&tmp)
        .assert()
        .success()
        .stderr(predicate::str::contains("no issues found"));
}

#[test]
fn doctor_clean_project_json() {
    let tmp = init_tmp();
    create_task(&tmp, "Healthy task");

    let report = doctor_json(&tmp, false);
    assert_eq!(report["summary"]["total"], 0);
    assert!(report["findings"].as_array().unwrap().is_empty());
}

// --- Dangling parent ---

#[test]
fn doctor_detects_dangling_parent_missing() {
    let tmp = init_tmp();

    // Import a task whose parent ULID doesn't exist.
    import_jsonl(
        &tmp,
        &[&format!(
            r#"{{"id": "{TASK01}", "title": "Orphan", "parent": "{GHOST1}", "created_at": "2026-01-01T00:00:00Z", "updated_at": "2026-01-01T00:00:00Z"}}"#
        )],
    );

    let report = doctor_json(&tmp, false);
    assert_eq!(report["summary"]["dangling_parents"], 1);
    assert_eq!(report["summary"]["total"], 1);
    assert_eq!(report["findings"][0]["kind"], "dangling_parent");
    assert!(!report["findings"][0]["fixed"].as_bool().unwrap());
}

#[test]
fn doctor_detects_dangling_parent_tombstoned() {
    let tmp = init_tmp();

    // Import a tombstoned parent and a live child still pointing at it.
    import_jsonl(
        &tmp,
        &[
            &format!(
                r#"{{"id": "{TASK21}", "title": "Dead parent", "deleted_at": "2026-01-01T00:00:00Z", "created_at": "2026-01-01T00:00:00Z", "updated_at": "2026-01-01T00:00:00Z"}}"#
            ),
            &format!(
                r#"{{"id": "{TASK22}", "title": "Orphaned child", "parent": "{TASK21}", "created_at": "2026-01-01T00:00:00Z", "updated_at": "2026-01-01T00:00:00Z"}}"#
            ),
        ],
    );

    let report = doctor_json(&tmp, false);
    assert_eq!(report["summary"]["dangling_parents"], 1);
    assert!(report["findings"][0]["detail"]
        .as_str()
        .unwrap()
        .contains("tombstoned"));
}

#[test]
fn doctor_fix_clears_dangling_parent() {
    let tmp = init_tmp();

    import_jsonl(
        &tmp,
        &[&format!(
            r#"{{"id": "{TASK01}", "title": "Orphan", "parent": "{GHOST1}", "created_at": "2026-01-01T00:00:00Z", "updated_at": "2026-01-01T00:00:00Z"}}"#
        )],
    );

    let report = doctor_json(&tmp, true);
    assert_eq!(report["summary"]["fixed"], 1);
    assert!(report["findings"][0]["fixed"].as_bool().unwrap());

    // Re-run: should be clean now.
    let clean = doctor_json(&tmp, false);
    assert_eq!(clean["summary"]["total"], 0);
}

// --- Dangling blocker ---

#[test]
fn doctor_detects_dangling_blocker() {
    let tmp = init_tmp();

    import_jsonl(
        &tmp,
        &[&format!(
            r#"{{"id": "{TASK02}", "title": "Blocked", "blockers": ["{GHOST2}"], "created_at": "2026-01-01T00:00:00Z", "updated_at": "2026-01-01T00:00:00Z"}}"#
        )],
    );

    let report = doctor_json(&tmp, false);
    assert_eq!(report["summary"]["dangling_blockers"], 1);
    assert_eq!(report["findings"][0]["kind"], "dangling_blocker");
}

#[test]
fn doctor_fix_removes_dangling_blocker() {
    let tmp = init_tmp();

    import_jsonl(
        &tmp,
        &[&format!(
            r#"{{"id": "{TASK02}", "title": "Blocked", "blockers": ["{GHOST2}"], "created_at": "2026-01-01T00:00:00Z", "updated_at": "2026-01-01T00:00:00Z"}}"#
        )],
    );

    let report = doctor_json(&tmp, true);
    assert_eq!(report["summary"]["fixed"], 1);

    // Re-run: clean.
    let clean = doctor_json(&tmp, false);
    assert_eq!(clean["summary"]["total"], 0);
}

// --- Blocker cycle ---

#[test]
fn doctor_detects_blocker_cycle() {
    let tmp = init_tmp();

    // Import two tasks that block each other (cycle bypassing dep add's check).
    import_jsonl(
        &tmp,
        &[
            &format!(
                r#"{{"id": "{TASK03}", "title": "Task A", "blockers": ["{TASK04}"], "created_at": "2026-01-01T00:00:00Z", "updated_at": "2026-01-01T00:00:00Z"}}"#
            ),
            &format!(
                r#"{{"id": "{TASK04}", "title": "Task B", "blockers": ["{TASK03}"], "created_at": "2026-01-01T00:00:00Z", "updated_at": "2026-01-01T00:00:00Z"}}"#
            ),
        ],
    );

    let report = doctor_json(&tmp, false);
    assert_eq!(report["summary"]["blocker_cycles"], 1);
    assert!(report["findings"][0]["active"].as_bool().unwrap());
}

#[test]
fn doctor_fix_breaks_blocker_cycle() {
    let tmp = init_tmp();

    import_jsonl(
        &tmp,
        &[
            &format!(
                r#"{{"id": "{TASK03}", "title": "Task A", "blockers": ["{TASK04}"], "created_at": "2026-01-01T00:00:00Z", "updated_at": "2026-01-01T00:00:00Z"}}"#
            ),
            &format!(
                r#"{{"id": "{TASK04}", "title": "Task B", "blockers": ["{TASK03}"], "created_at": "2026-01-01T00:00:00Z", "updated_at": "2026-01-01T00:00:00Z"}}"#
            ),
        ],
    );

    let report = doctor_json(&tmp, true);
    assert_eq!(report["summary"]["fixed"], 1);

    // Re-run: clean.
    let clean = doctor_json(&tmp, false);
    assert_eq!(clean["summary"]["total"], 0);
}

#[test]
fn doctor_blocker_cycle_inert_when_one_node_closed() {
    let tmp = init_tmp();

    // Create two tasks that block each other, but one is closed.
    import_jsonl(
        &tmp,
        &[
            &format!(
                r#"{{"id": "{TASK05}", "title": "Open task", "blockers": ["{TASK06}"], "created_at": "2026-01-01T00:00:00Z", "updated_at": "2026-01-01T00:00:00Z"}}"#
            ),
            &format!(
                r#"{{"id": "{TASK06}", "title": "Closed task", "status": "closed", "blockers": ["{TASK05}"], "created_at": "2026-01-01T00:00:00Z", "updated_at": "2026-01-01T00:00:00Z"}}"#
            ),
        ],
    );

    let report = doctor_json(&tmp, false);
    assert_eq!(report["summary"]["blocker_cycles"], 1);
    assert!(!report["findings"][0]["active"].as_bool().unwrap());
}

#[test]
fn doctor_fix_skips_inert_blocker_cycle() {
    let tmp = init_tmp();

    import_jsonl(
        &tmp,
        &[
            &format!(
                r#"{{"id": "{TASK05}", "title": "Open task", "blockers": ["{TASK06}"], "created_at": "2026-01-01T00:00:00Z", "updated_at": "2026-01-01T00:00:00Z"}}"#
            ),
            &format!(
                r#"{{"id": "{TASK06}", "title": "Closed task", "status": "closed", "blockers": ["{TASK05}"], "created_at": "2026-01-01T00:00:00Z", "updated_at": "2026-01-01T00:00:00Z"}}"#
            ),
        ],
    );

    let report = doctor_json(&tmp, true);
    // Inert cycle is reported but not fixed.
    assert_eq!(report["summary"]["blocker_cycles"], 1);
    assert_eq!(report["summary"]["fixed"], 0);
    assert!(!report["findings"][0]["fixed"].as_bool().unwrap());
}

// --- Parent cycle ---

#[test]
fn doctor_detects_parent_cycle() {
    let tmp = init_tmp();

    import_jsonl(
        &tmp,
        &[
            &format!(
                r#"{{"id": "{TASK07}", "title": "Task E", "parent": "{TASK08}", "created_at": "2026-01-01T00:00:00Z", "updated_at": "2026-01-01T00:00:00Z"}}"#
            ),
            &format!(
                r#"{{"id": "{TASK08}", "title": "Task F", "parent": "{TASK07}", "created_at": "2026-01-01T00:00:00Z", "updated_at": "2026-01-01T00:00:00Z"}}"#
            ),
        ],
    );

    let report = doctor_json(&tmp, false);
    assert_eq!(report["summary"]["parent_cycles"], 1);
}

#[test]
fn doctor_fix_breaks_parent_cycle() {
    let tmp = init_tmp();

    import_jsonl(
        &tmp,
        &[
            &format!(
                r#"{{"id": "{TASK07}", "title": "Task E", "parent": "{TASK08}", "created_at": "2026-01-01T00:00:00Z", "updated_at": "2026-01-01T00:00:00Z"}}"#
            ),
            &format!(
                r#"{{"id": "{TASK08}", "title": "Task F", "parent": "{TASK07}", "created_at": "2026-01-01T00:00:00Z", "updated_at": "2026-01-01T00:00:00Z"}}"#
            ),
        ],
    );

    let report = doctor_json(&tmp, true);
    assert_eq!(report["summary"]["fixed"], 1);

    // The lower ULID (TASK07) should have its parent cleared.
    let out = td(&tmp)
        .args(["--json", "show", TASK07])
        .current_dir(&tmp)
        .output()
        .unwrap();
    let task: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert!(
        task["parent"].is_null(),
        "expected parent to be cleared (null or absent), got: {}",
        task["parent"]
    );

    // Re-run: clean.
    let clean = doctor_json(&tmp, false);
    assert_eq!(clean["summary"]["total"], 0);
}

// --- Transitive blocker cycle ---

#[test]
fn doctor_detects_transitive_blocker_cycle() {
    let tmp = init_tmp();

    // Three-node cycle: A → B → C → A
    import_jsonl(
        &tmp,
        &[
            &format!(
                r#"{{"id": "{TASK0A}", "title": "Task A", "blockers": ["{TASK0B}"], "created_at": "2026-01-01T00:00:00Z", "updated_at": "2026-01-01T00:00:00Z"}}"#
            ),
            &format!(
                r#"{{"id": "{TASK0B}", "title": "Task B", "blockers": ["{TASK0C}"], "created_at": "2026-01-01T00:00:00Z", "updated_at": "2026-01-01T00:00:00Z"}}"#
            ),
            &format!(
                r#"{{"id": "{TASK0C}", "title": "Task C", "blockers": ["{TASK0A}"], "created_at": "2026-01-01T00:00:00Z", "updated_at": "2026-01-01T00:00:00Z"}}"#
            ),
        ],
    );

    let report = doctor_json(&tmp, false);
    assert_eq!(report["summary"]["blocker_cycles"], 1);
    assert!(report["findings"][0]["active"].as_bool().unwrap());
}

#[test]
fn doctor_fix_breaks_transitive_blocker_cycle() {
    let tmp = init_tmp();

    import_jsonl(
        &tmp,
        &[
            &format!(
                r#"{{"id": "{TASK0A}", "title": "Task A", "blockers": ["{TASK0B}"], "created_at": "2026-01-01T00:00:00Z", "updated_at": "2026-01-01T00:00:00Z"}}"#
            ),
            &format!(
                r#"{{"id": "{TASK0B}", "title": "Task B", "blockers": ["{TASK0C}"], "created_at": "2026-01-01T00:00:00Z", "updated_at": "2026-01-01T00:00:00Z"}}"#
            ),
            &format!(
                r#"{{"id": "{TASK0C}", "title": "Task C", "blockers": ["{TASK0A}"], "created_at": "2026-01-01T00:00:00Z", "updated_at": "2026-01-01T00:00:00Z"}}"#
            ),
        ],
    );

    let report = doctor_json(&tmp, true);
    assert_eq!(report["summary"]["fixed"], 1);

    // Re-run: clean.
    let clean = doctor_json(&tmp, false);
    assert_eq!(clean["summary"]["total"], 0);
}

// --- Multiple issues ---

#[test]
fn doctor_detects_multiple_issues() {
    let tmp = init_tmp();

    import_jsonl(
        &tmp,
        &[
            // Dangling parent.
            &format!(
                r#"{{"id": "{TASK10}", "title": "Orphan", "parent": "{GHOST3}", "created_at": "2026-01-01T00:00:00Z", "updated_at": "2026-01-01T00:00:00Z"}}"#
            ),
            // Dangling blocker.
            &format!(
                r#"{{"id": "{TASK11}", "title": "Bad dep", "blockers": ["{GHOST4}"], "created_at": "2026-01-01T00:00:00Z", "updated_at": "2026-01-01T00:00:00Z"}}"#
            ),
            // Blocker cycle.
            &format!(
                r#"{{"id": "{TASK12}", "title": "Cycle A", "blockers": ["{TASK13}"], "created_at": "2026-01-01T00:00:00Z", "updated_at": "2026-01-01T00:00:00Z"}}"#
            ),
            &format!(
                r#"{{"id": "{TASK13}", "title": "Cycle B", "blockers": ["{TASK12}"], "created_at": "2026-01-01T00:00:00Z", "updated_at": "2026-01-01T00:00:00Z"}}"#
            ),
        ],
    );

    let report = doctor_json(&tmp, false);
    assert_eq!(report["summary"]["dangling_parents"], 1);
    assert_eq!(report["summary"]["dangling_blockers"], 1);
    assert_eq!(report["summary"]["blocker_cycles"], 1);
    assert_eq!(report["summary"]["total"], 3);
}

#[test]
fn doctor_fix_repairs_all_issues_at_once() {
    let tmp = init_tmp();

    import_jsonl(
        &tmp,
        &[
            &format!(
                r#"{{"id": "{TASK10}", "title": "Orphan", "parent": "{GHOST3}", "created_at": "2026-01-01T00:00:00Z", "updated_at": "2026-01-01T00:00:00Z"}}"#
            ),
            &format!(
                r#"{{"id": "{TASK11}", "title": "Bad dep", "blockers": ["{GHOST4}"], "created_at": "2026-01-01T00:00:00Z", "updated_at": "2026-01-01T00:00:00Z"}}"#
            ),
            &format!(
                r#"{{"id": "{TASK12}", "title": "Cycle A", "blockers": ["{TASK13}"], "created_at": "2026-01-01T00:00:00Z", "updated_at": "2026-01-01T00:00:00Z"}}"#
            ),
            &format!(
                r#"{{"id": "{TASK13}", "title": "Cycle B", "blockers": ["{TASK12}"], "created_at": "2026-01-01T00:00:00Z", "updated_at": "2026-01-01T00:00:00Z"}}"#
            ),
        ],
    );

    let report = doctor_json(&tmp, true);
    assert_eq!(report["summary"]["total"], 3);
    assert_eq!(report["summary"]["fixed"], 3);

    let clean = doctor_json(&tmp, false);
    assert_eq!(clean["summary"]["total"], 0);
}

// --- Without --fix, doctor is read-only ---

#[test]
fn doctor_without_fix_does_not_modify_data() {
    let tmp = init_tmp();

    import_jsonl(
        &tmp,
        &[&format!(
            r#"{{"id": "{TASK20}", "title": "Orphan", "parent": "{GHOST5}", "created_at": "2026-01-01T00:00:00Z", "updated_at": "2026-01-01T00:00:00Z"}}"#
        )],
    );

    // Run doctor twice without --fix.
    let first = doctor_json(&tmp, false);
    let second = doctor_json(&tmp, false);

    // Same findings both times: nothing changed.
    // Compare findings arrays specifically (not full report to avoid timestamp noise).
    assert_eq!(
        first["findings"], second["findings"],
        "Running doctor without --fix should be idempotent"
    );
    assert_eq!(first["summary"]["fixed"], 0);
    assert_eq!(second["summary"]["fixed"], 0);
}

// --- Human-readable output ---

#[test]
fn doctor_human_output_suggests_fix() {
    let tmp = init_tmp();

    import_jsonl(
        &tmp,
        &[&format!(
            r#"{{"id": "{TASK30}", "title": "Bad parent", "parent": "{GHOST6}", "created_at": "2026-01-01T00:00:00Z", "updated_at": "2026-01-01T00:00:00Z"}}"#
        )],
    );

    td(&tmp)
        .args(["doctor"])
        .current_dir(&tmp)
        .assert()
        .success()
        .stderr(predicate::str::contains("dangling parent"))
        .stderr(predicate::str::contains("Run with --fix to repair"));
}

#[test]
fn doctor_human_output_shows_fixed() {
    let tmp = init_tmp();

    import_jsonl(
        &tmp,
        &[&format!(
            r#"{{"id": "{TASK30}", "title": "Bad parent", "parent": "{GHOST6}", "created_at": "2026-01-01T00:00:00Z", "updated_at": "2026-01-01T00:00:00Z"}}"#
        )],
    );

    td(&tmp)
        .args(["doctor", "--fix"])
        .current_dir(&tmp)
        .assert()
        .success()
        .stderr(predicate::str::contains("fixed:"));
}
