use assert_cmd::Command;
use tempfile::TempDir;

fn td(home: &TempDir) -> Command {
    let mut cmd = Command::cargo_bin("td").unwrap();
    cmd.env("HOME", home.path());
    cmd
}

#[test]
fn compact_cleans_delta_files() {
    let tmp = TempDir::new().unwrap();
    td(&tmp)
        .args(["init", "main"])
        .current_dir(&tmp)
        .assert()
        .success();

    // Generate some deltas
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

    let project_dir = tmp.path().join(".local/share/td/projects/main");
    let changes_dir = project_dir.join("changes");

    // Check deltas exist
    let deltas = std::fs::read_dir(&changes_dir).unwrap().count();
    assert!(deltas > 0, "Deltas should exist before compaction");

    // Tidy (formerly compact)
    td(&tmp).arg("tidy").current_dir(&tmp).assert().success();

    // Deltas are folded into the snapshot and removed.
    let deltas_after = std::fs::read_dir(&changes_dir).unwrap().count();
    assert_eq!(deltas_after, 0, "Compaction should clean up delta files");
}
