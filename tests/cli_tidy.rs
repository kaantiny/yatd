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
        .args(["project", "init", "main"])
        .current_dir(&tmp)
        .assert()
        .success();
    tmp
}

/// td tidy exists and reports what it did.
#[test]
fn tidy_reports_progress() {
    let tmp = init_tmp();

    td(&tmp)
        .args(["create", "Task 1"])
        .current_dir(&tmp)
        .assert()
        .success();
    td(&tmp)
        .args(["create", "Task 2"])
        .current_dir(&tmp)
        .assert()
        .success();

    td(&tmp)
        .arg("tidy")
        .current_dir(&tmp)
        .assert()
        .success()
        .stderr(predicate::str::contains("removed 2 delta file(s)"));
}

/// After td tidy, the changes/ directory exists but contains no delta files.
#[test]
fn tidy_empties_changes_dir() {
    let tmp = init_tmp();

    td(&tmp)
        .args(["create", "Task A"])
        .current_dir(&tmp)
        .assert()
        .success();

    let project_dir = tmp.path().join(".local/share/td/projects/main");
    let changes_dir = project_dir.join("changes");

    let before = std::fs::read_dir(&changes_dir).unwrap().count();
    assert!(before > 0, "deltas should exist before tidy");

    td(&tmp).arg("tidy").current_dir(&tmp).assert().success();

    // changes/ should still be there (ready for new deltas)…
    assert!(changes_dir.exists(), "changes/ must still exist after tidy");
    // …but empty.
    let after = std::fs::read_dir(&changes_dir).unwrap().count();
    assert_eq!(after, 0, "tidy should leave changes/ empty");
}

/// All tasks created before td tidy are still visible afterwards.
#[test]
fn tidy_preserves_tasks() {
    let tmp = init_tmp();

    td(&tmp)
        .args(["create", "Keep me"])
        .current_dir(&tmp)
        .assert()
        .success();
    td(&tmp)
        .args(["create", "Keep me too"])
        .current_dir(&tmp)
        .assert()
        .success();

    td(&tmp).arg("tidy").current_dir(&tmp).assert().success();

    td(&tmp)
        .arg("list")
        .current_dir(&tmp)
        .assert()
        .success()
        .stdout(predicate::str::contains("Keep me"))
        .stdout(predicate::str::contains("Keep me too"));
}

/// Running td tidy twice in a row is idempotent: the second run has no deltas
/// to remove, succeeds, and reports 0 files removed.
#[test]
fn tidy_is_idempotent() {
    let tmp = init_tmp();

    td(&tmp)
        .args(["create", "Some task"])
        .current_dir(&tmp)
        .assert()
        .success();

    td(&tmp).arg("tidy").current_dir(&tmp).assert().success();

    td(&tmp)
        .arg("tidy")
        .current_dir(&tmp)
        .assert()
        .success()
        .stderr(predicate::str::contains("removed 0 delta file(s)"));
}

/// Crash-recovery: if a previous tidy renamed changes/ to changes.compacting.X/
/// but crashed before finishing, the next tidy should absorb those deltas into
/// the snapshot and remove the orphaned directory.
#[test]
fn tidy_recovers_orphaned_compacting_dir() {
    let tmp = init_tmp();

    td(&tmp)
        .args(["create", "Orphaned task"])
        .current_dir(&tmp)
        .assert()
        .success();

    let project_dir = tmp.path().join(".local/share/td/projects/main");
    let changes_dir = project_dir.join("changes");

    // Simulate a crash after phase 1: rename changes/ to changes.compacting.X/
    // without creating a fresh changes/ or writing the new snapshot.
    let compacting_dir = project_dir.join("changes.compacting.01JNFAKEULID0000000000000");
    std::fs::rename(&changes_dir, &compacting_dir)
        .expect("simulate crash: rename changes/ to compacting dir");

    // No changes/ exists now — a subsequent write would create it via
    // create_dir_all, but let's leave that to tidy to exercise recovery.

    // tidy must succeed and preserve the orphaned task.
    td(&tmp).arg("tidy").current_dir(&tmp).assert().success();

    // The orphaned compacting dir must be gone.
    assert!(
        !compacting_dir.exists(),
        "tidy should remove the orphaned changes.compacting.* dir"
    );

    // The task from the orphaned delta must still be accessible.
    td(&tmp)
        .arg("list")
        .current_dir(&tmp)
        .assert()
        .success()
        .stdout(predicate::str::contains("Orphaned task"));
}

/// td compact is gone; using it should produce an error.
#[test]
fn compact_command_removed() {
    let tmp = init_tmp();

    td(&tmp).arg("compact").current_dir(&tmp).assert().failure();
}
