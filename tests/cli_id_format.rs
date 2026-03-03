//! Tests that all user-visible task ID output uses the short `td-XXXXXXX` form.
//!
//! IDs emitted in JSON, human output, and cross-referencing fields (parent,
//! blockers, log entry IDs) must all consistently use the short form. The one
//! exception is `export`, which must emit full ULIDs so that `import` can
//! round-trip data losslessly.

use assert_cmd::cargo::cargo_bin_cmd;
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

/// Assert a string matches the `td-XXXXXXX` pattern (3-char prefix + 7 uppercase alphanumeric).
fn assert_short_id(id: &str) {
    assert!(
        id.starts_with("td-") && id.len() == 10,
        "expected short ID like td-XXXXXXX, got: {id}"
    );
}

// ── create ───────────────────────────────────────────────────────────

#[test]
fn create_json_emits_short_id() {
    let tmp = init_tmp();
    let id = create_task(&tmp, "Test task");
    assert_short_id(&id);
}

#[test]
fn create_json_parent_is_short_id() {
    let tmp = init_tmp();
    let parent = create_task(&tmp, "Parent");

    let out = td(&tmp)
        .args(["--json", "create", "Child", "--parent", &parent])
        .current_dir(&tmp)
        .output()
        .unwrap();
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    let parent_field = v["parent"].as_str().unwrap();
    assert_short_id(parent_field);
}

// ── show ─────────────────────────────────────────────────────────────

#[test]
fn show_json_emits_short_ids() {
    let tmp = init_tmp();
    let id = create_task(&tmp, "Show me");

    let out = td(&tmp)
        .args(["--json", "show", &id])
        .current_dir(&tmp)
        .output()
        .unwrap();
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_short_id(v["id"].as_str().unwrap());
}

#[test]
fn show_json_blockers_are_short_ids() {
    let tmp = init_tmp();
    let a = create_task(&tmp, "Blocked");
    let b = create_task(&tmp, "Blocker");

    td(&tmp)
        .args(["dep", "add", &a, &b])
        .current_dir(&tmp)
        .assert()
        .success();

    let out = td(&tmp)
        .args(["--json", "show", &a])
        .current_dir(&tmp)
        .output()
        .unwrap();
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    for blocker in v["blockers"].as_array().unwrap() {
        assert_short_id(blocker.as_str().unwrap());
    }
}

// ── list ─────────────────────────────────────────────────────────────

#[test]
fn list_json_emits_short_ids() {
    let tmp = init_tmp();
    create_task(&tmp, "Listed");

    let out = td(&tmp)
        .args(["--json", "list"])
        .current_dir(&tmp)
        .output()
        .unwrap();
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    for task in v.as_array().unwrap() {
        assert_short_id(task["id"].as_str().unwrap());
    }
}

// ── done ─────────────────────────────────────────────────────────────

#[test]
fn done_json_emits_short_id() {
    let tmp = init_tmp();
    let id = create_task(&tmp, "Close me");

    let out = td(&tmp)
        .args(["--json", "done", &id])
        .current_dir(&tmp)
        .output()
        .unwrap();
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_short_id(v[0]["id"].as_str().unwrap());
}

#[test]
fn done_human_emits_short_id() {
    let tmp = init_tmp();
    let id = create_task(&tmp, "Close me too");

    let out = td(&tmp)
        .args(["done", &id])
        .current_dir(&tmp)
        .output()
        .unwrap();
    let stdout = String::from_utf8(out.stdout).unwrap();
    // The human output should contain the short ID, not a 26-char ULID.
    assert!(
        stdout.contains(&id),
        "human output should contain short ID {id}, got: {stdout}"
    );
}

// ── reopen ───────────────────────────────────────────────────────────

#[test]
fn reopen_json_emits_short_id() {
    let tmp = init_tmp();
    let id = create_task(&tmp, "Reopen me");

    td(&tmp)
        .args(["done", &id])
        .current_dir(&tmp)
        .assert()
        .success();

    let out = td(&tmp)
        .args(["--json", "reopen", &id])
        .current_dir(&tmp)
        .output()
        .unwrap();
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_short_id(v[0]["id"].as_str().unwrap());
}

#[test]
fn reopen_human_emits_short_id() {
    let tmp = init_tmp();
    let id = create_task(&tmp, "Reopen me too");

    td(&tmp)
        .args(["done", &id])
        .current_dir(&tmp)
        .assert()
        .success();

    let out = td(&tmp)
        .args(["reopen", &id])
        .current_dir(&tmp)
        .output()
        .unwrap();
    let stdout = String::from_utf8(out.stdout).unwrap();
    assert!(
        stdout.contains(&id),
        "human output should contain short ID {id}, got: {stdout}"
    );
}

// ── dep ──────────────────────────────────────────────────────────────

#[test]
fn dep_add_json_emits_short_ids() {
    let tmp = init_tmp();
    let a = create_task(&tmp, "Child");
    let b = create_task(&tmp, "Parent");

    let out = td(&tmp)
        .args(["--json", "dep", "add", &a, &b])
        .current_dir(&tmp)
        .output()
        .unwrap();
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_short_id(v["child"].as_str().unwrap());
    assert_short_id(v["blocker"].as_str().unwrap());
}

// ── label ────────────────────────────────────────────────────────────

#[test]
fn label_add_json_emits_short_id() {
    let tmp = init_tmp();
    let id = create_task(&tmp, "Label me");

    let out = td(&tmp)
        .args(["--json", "label", "add", &id, "urgent"])
        .current_dir(&tmp)
        .output()
        .unwrap();
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_short_id(v["id"].as_str().unwrap());
}

// ── rm ───────────────────────────────────────────────────────────────

#[test]
fn rm_json_emits_short_ids() {
    let tmp = init_tmp();
    let id = create_task(&tmp, "Delete me");

    let out = td(&tmp)
        .args(["--json", "rm", &id, "--force"])
        .current_dir(&tmp)
        .output()
        .unwrap();
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    for deleted in v["deleted_ids"].as_array().unwrap() {
        assert_short_id(deleted.as_str().unwrap());
    }
}

// ── log ──────────────────────────────────────────────────────────────

#[test]
fn log_json_entry_id_is_short() {
    let tmp = init_tmp();
    let id = create_task(&tmp, "Log target");

    let out = td(&tmp)
        .args(["--json", "log", &id, "a note"])
        .current_dir(&tmp)
        .output()
        .unwrap();
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_short_id(v["id"].as_str().unwrap());
}

// ── next ─────────────────────────────────────────────────────────────

#[test]
fn next_json_emits_short_ids() {
    let tmp = init_tmp();
    create_task(&tmp, "A task");

    let out = td(&tmp)
        .args(["--json", "next"])
        .current_dir(&tmp)
        .output()
        .unwrap();
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    for entry in v.as_array().unwrap() {
        assert_short_id(entry["id"].as_str().unwrap());
    }
}

// ── search ───────────────────────────────────────────────────────────

#[test]
fn search_json_emits_short_ids() {
    let tmp = init_tmp();
    create_task(&tmp, "Searchable task");

    let out = td(&tmp)
        .args(["--json", "search", "Searchable"])
        .current_dir(&tmp)
        .output()
        .unwrap();
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    for task in v.as_array().unwrap() {
        assert_short_id(task["id"].as_str().unwrap());
    }
}

// ── ready ────────────────────────────────────────────────────────────

#[test]
fn ready_json_emits_short_ids() {
    let tmp = init_tmp();
    create_task(&tmp, "Ready task");

    let out = td(&tmp)
        .args(["--json", "ready"])
        .current_dir(&tmp)
        .output()
        .unwrap();
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    for task in v.as_array().unwrap() {
        assert_short_id(task["id"].as_str().unwrap());
    }
}

// ── update ───────────────────────────────────────────────────────────

#[test]
fn update_json_emits_short_id() {
    let tmp = init_tmp();
    let id = create_task(&tmp, "Update me");

    let out = td(&tmp)
        .args(["--json", "update", &id, "-t", "Updated"])
        .current_dir(&tmp)
        .output()
        .unwrap();
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_short_id(v["id"].as_str().unwrap());
}

// ── export/import round-trip ─────────────────────────────────────────

#[test]
fn export_emits_full_ulids_for_import() {
    let tmp = init_tmp();
    create_task(&tmp, "Exportable");

    let out = td(&tmp).arg("export").current_dir(&tmp).output().unwrap();
    let stdout = String::from_utf8(out.stdout).unwrap();
    let mut checked = false;
    for line in stdout.lines() {
        let v: serde_json::Value = serde_json::from_str(line).unwrap();
        let id = v["id"].as_str().unwrap();
        // Export IDs must be full 26-char ULIDs, not the short form.
        assert!(
            !id.starts_with("td-") && id.len() == 26,
            "export should emit full ULID, got: {id}"
        );
        checked = true;
    }
    assert!(checked, "export produced no output to check");
}

#[test]
fn export_import_round_trip_preserves_ids() {
    let tmp = init_tmp();
    create_task(&tmp, "Round trip");

    // Export from original.
    let export_out = td(&tmp).arg("export").current_dir(&tmp).output().unwrap();
    let exported = String::from_utf8(export_out.stdout).unwrap();
    let export_file = tmp.path().join("rt.jsonl");
    std::fs::write(&export_file, &exported).unwrap();

    // Import into fresh project.
    let tmp2 = TempDir::new().unwrap();
    td(&tmp2)
        .args(["project", "init", "mirror"])
        .current_dir(&tmp2)
        .assert()
        .success();
    td(&tmp2)
        .args(["import", export_file.to_str().unwrap()])
        .current_dir(&tmp2)
        .assert()
        .success();

    // The JSON list output in the new project should have valid short IDs.
    let out = td(&tmp2)
        .args(["--json", "list"])
        .current_dir(&tmp2)
        .output()
        .unwrap();
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_short_id(v[0]["id"].as_str().unwrap());
}

// ── cross-command consistency ────────────────────────────────────────

#[test]
fn json_ids_are_usable_as_input_across_commands() {
    let tmp = init_tmp();
    let id = create_task(&tmp, "Cross-command");
    assert_short_id(&id);

    // The short ID from create --json should work as input to show.
    let out = td(&tmp)
        .args(["--json", "show", &id])
        .current_dir(&tmp)
        .output()
        .unwrap();
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(v["id"].as_str().unwrap(), &id);

    // And to done.
    td(&tmp)
        .args(["done", &id])
        .current_dir(&tmp)
        .assert()
        .success();

    // And to reopen.
    td(&tmp)
        .args(["reopen", &id])
        .current_dir(&tmp)
        .assert()
        .success();
}

// ── show --json logs contain short IDs ───────────────────────────────

#[test]
fn show_json_log_entry_ids_are_short() {
    let tmp = init_tmp();
    let id = create_task(&tmp, "Log host");

    td(&tmp)
        .args(["log", &id, "first note"])
        .current_dir(&tmp)
        .assert()
        .success();

    let out = td(&tmp)
        .args(["--json", "show", &id])
        .current_dir(&tmp)
        .output()
        .unwrap();
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    for entry in v["logs"].as_array().unwrap() {
        assert_short_id(entry["id"].as_str().unwrap());
    }
}
