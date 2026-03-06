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

fn get_task_json(dir: &TempDir, id: &str) -> serde_json::Value {
    let out = td(dir)
        .args(["--json", "show", id])
        .current_dir(dir)
        .output()
        .unwrap();
    serde_json::from_slice(&out.stdout).unwrap()
}

// ── update ───────────────────────────────────────────────────────────

#[test]
fn update_changes_status() {
    let tmp = init_tmp();
    let id = create_task(&tmp, "In progress");

    td(&tmp)
        .args(["update", &id, "-s", "in_progress"])
        .current_dir(&tmp)
        .assert()
        .success()
        .stdout(predicate::str::contains("updated"));

    let t = get_task_json(&tmp, &id);
    assert_eq!(t["status"].as_str().unwrap(), "in_progress");
}

#[test]
fn update_changes_priority() {
    let tmp = init_tmp();
    let id = create_task(&tmp, "Reprioritise");

    td(&tmp)
        .args(["update", &id, "-p", "high"])
        .current_dir(&tmp)
        .assert()
        .success();

    let t = get_task_json(&tmp, &id);
    assert_eq!(t["priority"].as_str().unwrap(), "high");
}

#[test]
fn update_changes_title() {
    let tmp = init_tmp();
    let id = create_task(&tmp, "Old title");

    td(&tmp)
        .args(["update", &id, "-t", "New title"])
        .current_dir(&tmp)
        .assert()
        .success();

    let t = get_task_json(&tmp, &id);
    assert_eq!(t["title"].as_str().unwrap(), "New title");
}

#[test]
fn update_changes_description() {
    let tmp = init_tmp();
    let id = create_task(&tmp, "Describe me");

    td(&tmp)
        .args(["update", &id, "-d", "Now with details"])
        .current_dir(&tmp)
        .assert()
        .success();

    let t = get_task_json(&tmp, &id);
    assert_eq!(t["description"].as_str().unwrap(), "Now with details");
}

#[test]
fn update_json_returns_task() {
    let tmp = init_tmp();
    let id = create_task(&tmp, "JSON update");

    let out = td(&tmp)
        .args(["--json", "update", &id, "-p", "high"])
        .current_dir(&tmp)
        .output()
        .unwrap();
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(v["priority"].as_str().unwrap(), "high");
}

#[test]
fn update_changes_effort() {
    let tmp = init_tmp();
    let id = create_task(&tmp, "Re-estimate");

    td(&tmp)
        .args(["update", &id, "-e", "high"])
        .current_dir(&tmp)
        .assert()
        .success();

    let t = get_task_json(&tmp, &id);
    assert_eq!(t["effort"].as_str().unwrap(), "high");
}

// ── done ─────────────────────────────────────────────────────────────

#[test]
fn done_closes_task() {
    let tmp = init_tmp();
    let id = create_task(&tmp, "Close me");

    td(&tmp)
        .args(["done", &id])
        .current_dir(&tmp)
        .assert()
        .success()
        .stdout(predicate::str::contains("closed"));

    let t = get_task_json(&tmp, &id);
    assert_eq!(t["status"].as_str().unwrap(), "closed");
}

#[test]
fn done_closes_multiple_tasks() {
    let tmp = init_tmp();
    let id1 = create_task(&tmp, "First");
    let id2 = create_task(&tmp, "Second");

    td(&tmp)
        .args(["done", &id1, &id2])
        .current_dir(&tmp)
        .assert()
        .success();

    assert_eq!(get_task_json(&tmp, &id1)["status"], "closed");
    assert_eq!(get_task_json(&tmp, &id2)["status"], "closed");
}

// ── reopen ───────────────────────────────────────────────────────────

#[test]
fn reopen_reopens_closed_task() {
    let tmp = init_tmp();
    let id = create_task(&tmp, "Reopen me");

    td(&tmp)
        .args(["done", &id])
        .current_dir(&tmp)
        .assert()
        .success();
    assert_eq!(get_task_json(&tmp, &id)["status"], "closed");

    td(&tmp)
        .args(["reopen", &id])
        .current_dir(&tmp)
        .assert()
        .success()
        .stdout(predicate::str::contains("reopened"));

    assert_eq!(get_task_json(&tmp, &id)["status"], "open");
}

// ── editor fallback ───────────────────────────────────────────────────────────

#[test]
fn update_via_editor_changes_title_and_desc() {
    // Bare `td update <id>` with TD_FORCE_EDITOR should open the editor
    // pre-populated and apply whatever the fake editor writes back.
    let tmp = init_tmp();
    let id = create_task(&tmp, "Original title");

    let fake_editor = "sh -c 'printf \"New title\\nNew description\" > \"$1\"' sh";

    td(&tmp)
        .args(["update", &id])
        .env("TD_FORCE_EDITOR", fake_editor)
        .current_dir(&tmp)
        .assert()
        .success();

    let t = get_task_json(&tmp, &id);
    assert_eq!(t["title"].as_str().unwrap(), "New title");
    assert_eq!(t["description"].as_str().unwrap(), "New description");
}

#[test]
fn update_via_editor_aborts_on_empty_file() {
    // If the fake editor leaves only comments, the update should be aborted.
    let tmp = init_tmp();
    let id = create_task(&tmp, "Stays the same");

    let fake_editor = "sh -c 'printf \"TD: comment only\\n\" > \"$1\"' sh";

    td(&tmp)
        .args(["update", &id])
        .env("TD_FORCE_EDITOR", fake_editor)
        .current_dir(&tmp)
        .assert()
        .failure()
        .stderr(predicate::str::contains("aborted"));

    // Title must be unchanged.
    let t = get_task_json(&tmp, &id);
    assert_eq!(t["title"].as_str().unwrap(), "Stays the same");
}

#[test]
fn update_via_editor_preserves_existing_content_as_template() {
    // The editor should be pre-populated with the task's current title and
    // description, so the user can edit rather than retype from scratch.
    let tmp = init_tmp();
    let id = create_task(&tmp, "Existing title");

    // Set description first.
    td(&tmp)
        .args(["update", &id, "-d", "Existing description"])
        .current_dir(&tmp)
        .assert()
        .success();

    // Fake editor: grep out the first non-comment, non-blank line (the title),
    // then write "title: <that line>" so we can assert it was pre-populated.
    let fake_editor = concat!(
        "sh -c '",
        r#"title=$(grep -v "^TD: " "$1" | grep -v "^[[:space:]]*$" | head -1); "#,
        r#"printf "title: %s" "$title" > "$1""#,
        "' sh"
    );

    td(&tmp)
        .args(["update", &id])
        .env("TD_FORCE_EDITOR", fake_editor)
        .current_dir(&tmp)
        .assert()
        .success();

    let t = get_task_json(&tmp, &id);
    // The fake editor wrote the existing title back with a prefix, proving it
    // received the pre-populated template.
    assert_eq!(t["title"].as_str().unwrap(), "title: Existing title");
}
