use assert_cmd::cargo::cargo_bin_cmd;
use loro::{ExportMode, LoroDoc, VersionVector};
use predicates::prelude::*;

#[test]
fn sync_help_shows_usage() {
    let mut cmd = cargo_bin_cmd!("td");
    cmd.args(["sync", "--help"]);
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("Wormhole code"));
}

#[test]
fn sync_invalid_code_format_fails() {
    let home = tempfile::tempdir().unwrap();
    let cwd = tempfile::tempdir().unwrap();

    cargo_bin_cmd!("td")
        .args(["project", "init", "synctest"])
        .current_dir(cwd.path())
        .env("HOME", home.path())
        .assert()
        .success();

    let mut cmd = cargo_bin_cmd!("td");
    cmd.args(["sync", "not-a-valid-code"])
        .current_dir(cwd.path())
        .env("HOME", home.path());
    cmd.assert().failure();
}

/// Two peers sync over a real wormhole connection.
///
/// Setup: both stores share the same project_id (simulating a project
/// that was cloned to a second machine).  Each side creates a task the
/// other doesn't have.  After sync, both should see both tasks.
#[test]
fn sync_exchanges_tasks_between_peers() {
    use std::fs;
    use yatd::db;

    let home_a = tempfile::tempdir().unwrap();
    let cwd_a = tempfile::tempdir().unwrap();
    let home_b = tempfile::tempdir().unwrap();
    let cwd_b = tempfile::tempdir().unwrap();

    // --- Set up peer A: init a project and create a task ---
    std::env::set_var("HOME", home_a.path());
    let store_a = db::init(cwd_a.path(), "shared").unwrap();
    let id_a = db::gen_id();
    store_a
        .apply_and_persist(|doc| {
            let tasks = doc.get_map("tasks");
            let task = db::insert_task_map(&tasks, &id_a)?;
            task.insert("title", "task from A")?;
            task.insert("description", "")?;
            task.insert("type", "task")?;
            task.insert("priority", "medium")?;
            task.insert("status", "open")?;
            task.insert("effort", "medium")?;
            task.insert("parent", "")?;
            task.insert("created_at", db::now_utc())?;
            task.insert("updated_at", db::now_utc())?;
            task.insert("deleted_at", "")?;
            task.insert_container("labels", loro::LoroMap::new())?;
            task.insert_container("blockers", loro::LoroMap::new())?;
            task.insert_container("logs", loro::LoroMap::new())?;
            Ok(())
        })
        .unwrap();

    // --- Set up peer B: clone from A's snapshot, then add its own task ---
    //
    // Copy A's project directory so B has the same project_id and
    // initial state, then create a separate device_id for B.
    let data_a = home_a.path().join(".local/share/td/projects/shared");
    let data_b = home_b.path().join(".local/share/td/projects/shared");
    fs::create_dir_all(data_b.join("changes")).unwrap();
    // Copy only the base snapshot — A's change deltas stay with A.
    fs::copy(data_a.join("base.loro"), data_b.join("base.loro")).unwrap();

    // Write a binding so db::open from cwd_b resolves to "shared".
    let binding_dir = home_b.path().join(".local/share/td");
    fs::create_dir_all(&binding_dir).unwrap();
    let canonical_b = fs::canonicalize(cwd_b.path()).unwrap();
    let bindings = serde_json::json!({
        "bindings": {
            canonical_b.to_string_lossy().to_string(): "shared"
        }
    });
    fs::write(
        binding_dir.join("bindings.json"),
        serde_json::to_string_pretty(&bindings).unwrap(),
    )
    .unwrap();

    std::env::set_var("HOME", home_b.path());
    let store_b = db::open(cwd_b.path()).unwrap();
    let id_b = db::gen_id();
    store_b
        .apply_and_persist(|doc| {
            let tasks = doc.get_map("tasks");
            let task = db::insert_task_map(&tasks, &id_b)?;
            task.insert("title", "task from B")?;
            task.insert("description", "")?;
            task.insert("type", "task")?;
            task.insert("priority", "high")?;
            task.insert("status", "open")?;
            task.insert("effort", "low")?;
            task.insert("parent", "")?;
            task.insert("created_at", db::now_utc())?;
            task.insert("updated_at", db::now_utc())?;
            task.insert("deleted_at", "")?;
            task.insert_container("labels", loro::LoroMap::new())?;
            task.insert_container("blockers", loro::LoroMap::new())?;
            task.insert_container("logs", loro::LoroMap::new())?;
            Ok(())
        })
        .unwrap();

    // Verify pre-sync: A has 1 task, B has 2 (init snapshot + its own).
    // A's delta hasn't been applied to B's snapshot yet, so B only sees
    // tasks from the base snapshot plus its own delta.  Meanwhile A has
    // the base snapshot plus its own delta.
    let a_tasks_before = store_a.list_tasks().unwrap();
    let b_tasks_before = store_b.list_tasks().unwrap();
    assert_eq!(a_tasks_before.len(), 1, "A should have 1 task before sync");
    assert_eq!(b_tasks_before.len(), 1, "B should have 1 task before sync");

    // --- Sync via real wormhole ---
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        use magic_wormhole::{MailboxConnection, Wormhole};
        use yatd::cmd::sync::{exchange, wormhole_config};

        // Peer A creates the mailbox.
        let mailbox_a = MailboxConnection::create(wormhole_config(), 2)
            .await
            .unwrap();
        let code = mailbox_a.code().clone();

        // Peer B connects with the code.
        let mailbox_b = MailboxConnection::connect(wormhole_config(), code, false)
            .await
            .unwrap();

        // Both complete SPAKE2 key exchange concurrently.
        let (wormhole_a, wormhole_b) =
            tokio::try_join!(Wormhole::connect(mailbox_a), Wormhole::connect(mailbox_b),).unwrap();

        // Run the sync protocol on both sides concurrently.
        let (report_a, report_b) = tokio::try_join!(
            exchange(&store_a, wormhole_a),
            exchange(&store_b, wormhole_b),
        )
        .unwrap();

        assert!(report_a.imported, "A should have imported B's changes");
        assert!(report_b.imported, "B should have imported A's changes");
        assert!(report_a.sent_bytes > 0);
        assert!(report_b.sent_bytes > 0);
    });

    // --- Verify convergence ---
    let a_tasks = store_a.list_tasks().unwrap();
    let b_tasks = store_b.list_tasks().unwrap();

    assert_eq!(a_tasks.len(), 2, "A should have 2 tasks after sync");
    assert_eq!(b_tasks.len(), 2, "B should have 2 tasks after sync");

    let a_titles: Vec<&str> = a_tasks.iter().map(|t| t.title.as_str()).collect();
    let b_titles: Vec<&str> = b_tasks.iter().map(|t| t.title.as_str()).collect();
    assert!(a_titles.contains(&"task from A"));
    assert!(a_titles.contains(&"task from B"));
    assert!(b_titles.contains(&"task from A"));
    assert!(b_titles.contains(&"task from B"));
}

#[test]
fn try_open_returns_none_without_binding() {
    use yatd::db;

    let home = tempfile::tempdir().unwrap();
    let cwd = tempfile::tempdir().unwrap();

    std::env::set_var("HOME", home.path());
    assert!(
        db::try_open(cwd.path()).unwrap().is_none(),
        "expected no store when cwd is unbound and TD_PROJECT is unset"
    );
}

#[test]
fn bootstrap_from_peer_creates_openable_store() {
    use yatd::db;

    let home_a = tempfile::tempdir().unwrap();
    let cwd_a = tempfile::tempdir().unwrap();
    std::env::set_var("HOME", home_a.path());
    let source = db::init(cwd_a.path(), "shared").unwrap();

    let id = db::gen_id();
    source
        .apply_and_persist(|doc| {
            let tasks = doc.get_map("tasks");
            let task = db::insert_task_map(&tasks, &id)?;
            task.insert("title", "bootstrapped task")?;
            task.insert("description", "")?;
            task.insert("type", "task")?;
            task.insert("priority", "medium")?;
            task.insert("status", "open")?;
            task.insert("effort", "medium")?;
            task.insert("parent", "")?;
            task.insert("created_at", db::now_utc())?;
            task.insert("updated_at", db::now_utc())?;
            task.insert("deleted_at", "")?;
            task.insert_container("labels", loro::LoroMap::new())?;
            task.insert_container("blockers", loro::LoroMap::new())?;
            task.insert_container("logs", loro::LoroMap::new())?;
            Ok(())
        })
        .unwrap();

    let full_delta = source
        .doc()
        .export(ExportMode::updates(&VersionVector::default()))
        .unwrap();

    let home_b = tempfile::tempdir().unwrap();
    let root_b = home_b.path().join(".local/share/td");
    let store_b = db::Store::bootstrap_from_peer(&root_b, "shared", &full_delta).unwrap();

    assert_eq!(store_b.project_name(), "shared");
    assert!(
        root_b.join("projects/shared/base.loro").exists(),
        "bootstrap should persist a base snapshot"
    );

    let reopened = db::Store::open(&root_b, "shared").unwrap();
    let tasks = reopened.list_tasks().unwrap();
    assert_eq!(tasks.len(), 1);
    assert_eq!(tasks[0].title, "bootstrapped task");
}

#[test]
fn bootstrap_from_peer_rejects_missing_project_id() {
    use yatd::db;

    let doc = LoroDoc::new();
    doc.get_map("tasks");
    let meta = doc.get_map("meta");
    meta.insert("schema_version", 1i64).unwrap();
    doc.commit();

    let delta = doc
        .export(ExportMode::updates(&VersionVector::default()))
        .unwrap();

    let home = tempfile::tempdir().unwrap();
    let root = home.path().join(".local/share/td");
    let err = db::Store::bootstrap_from_peer(&root, "shared", &delta).unwrap_err();

    assert!(
        err.to_string()
            .contains("missing required project identity"),
        "unexpected error: {err:#}"
    );
    assert!(
        !root.join("projects/shared/base.loro").exists(),
        "bootstrap should not persist snapshot for invalid peer doc"
    );
}
