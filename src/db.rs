use anyhow::{anyhow, bail, Context, Result};
use loro::{Container, ExportMode, LoroDoc, LoroMap, PeerID, ValueOrContainer};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;
use std::fmt;
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

use fs2::FileExt;
use ulid::Ulid;

pub const PROJECT_ENV: &str = "TD_PROJECT";

pub(crate) const PROJECTS_DIR: &str = "projects";
const CHANGES_DIR: &str = "changes";
const BINDINGS_FILE: &str = "bindings.json";
const BASE_FILE: &str = "base.loro";
const TMP_SUFFIX: &str = ".tmp";
use crate::migrate;

/// Current UTC time in ISO 8601 format.
pub fn now_utc() -> String {
    chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string()
}

/// Lifecycle state for a task.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    Open,
    InProgress,
    Closed,
}

impl Status {
    fn as_str(self) -> &'static str {
        match self {
            Status::Open => "open",
            Status::InProgress => "in_progress",
            Status::Closed => "closed",
        }
    }

    fn parse(raw: &str) -> Result<Self> {
        match raw {
            "open" => Ok(Self::Open),
            "in_progress" => Ok(Self::InProgress),
            "closed" => Ok(Self::Closed),
            _ => bail!("invalid status '{raw}'"),
        }
    }
}

/// Priority for task ordering.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Priority {
    High,
    Medium,
    Low,
}

impl Priority {
    fn as_str(self) -> &'static str {
        match self {
            Priority::High => "high",
            Priority::Medium => "medium",
            Priority::Low => "low",
        }
    }

    fn parse(raw: &str) -> Result<Self> {
        match raw {
            "high" => Ok(Self::High),
            "medium" => Ok(Self::Medium),
            "low" => Ok(Self::Low),
            _ => bail!("invalid priority '{raw}'"),
        }
    }

    pub fn score(self) -> i32 {
        match self {
            Priority::High => 1,
            Priority::Medium => 2,
            Priority::Low => 3,
        }
    }
}

/// Estimated effort for a task.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Effort {
    Low,
    Medium,
    High,
}

impl Effort {
    fn as_str(self) -> &'static str {
        match self {
            Effort::Low => "low",
            Effort::Medium => "medium",
            Effort::High => "high",
        }
    }

    fn parse(raw: &str) -> Result<Self> {
        match raw {
            "low" => Ok(Self::Low),
            "medium" => Ok(Self::Medium),
            "high" => Ok(Self::High),
            _ => bail!("invalid effort '{raw}'"),
        }
    }

    pub fn score(self) -> i32 {
        match self {
            Effort::Low => 1,
            Effort::Medium => 2,
            Effort::High => 3,
        }
    }
}

/// A stable task identifier backed by a ULID.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize)]
#[serde(transparent)]
pub struct TaskId(String);

impl TaskId {
    pub fn new(id: Ulid) -> Self {
        Self(id.to_string())
    }

    pub fn parse(raw: &str) -> Result<Self> {
        let id = Ulid::from_string(raw).with_context(|| format!("invalid task id '{raw}'"))?;
        Ok(Self::new(id))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn short(&self) -> String {
        format!("td-{}", &self.0[self.0.len() - 7..])
    }

    /// Return a display-friendly short ID from a raw ULID string.
    pub fn display_id(raw: &str) -> String {
        let n = raw.len();
        if n > 7 {
            format!("td-{}", &raw[n - 7..])
        } else {
            format!("td-{raw}")
        }
    }
}

impl fmt::Display for TaskId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.short())
    }
}

/// A task log entry embedded in a task record.
#[derive(Debug, Clone, Serialize)]
pub struct LogEntry {
    pub id: TaskId,
    pub timestamp: String,
    pub message: String,
}

/// Hydrated task data from the CRDT document.
#[derive(Debug, Clone, Serialize)]
pub struct Task {
    pub id: TaskId,
    pub title: String,
    pub description: String,
    #[serde(rename = "type")]
    pub task_type: String,
    pub priority: Priority,
    pub status: Status,
    pub effort: Effort,
    pub parent: Option<TaskId>,
    pub created_at: String,
    pub updated_at: String,
    pub deleted_at: Option<String>,
    pub labels: Vec<String>,
    pub blockers: Vec<TaskId>,
    pub logs: Vec<LogEntry>,
}

/// Result type for partitioning blockers by task state.
#[derive(Debug, Default, Clone, Serialize)]
pub struct BlockerPartition {
    pub open: Vec<TaskId>,
    pub resolved: Vec<TaskId>,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
struct BindingsFile {
    #[serde(default)]
    bindings: BTreeMap<String, String>,
}

/// Storage wrapper around one project's Loro document and disk layout.
#[derive(Debug, Clone)]
pub struct Store {
    root: PathBuf,
    project: String,
    doc: LoroDoc,
}

impl Store {
    pub fn init(root: &Path, project: &str) -> Result<Self> {
        validate_project_name(project)?;
        let project_dir = project_dir(root, project);
        if project_dir.exists() {
            bail!("project '{project}' already exists");
        }
        fs::create_dir_all(project_dir.join(CHANGES_DIR))?;

        let doc = LoroDoc::new();
        doc.set_peer_id(load_or_create_device_peer_id(root)?)?;
        doc.get_map("tasks");

        let meta = doc.get_map("meta");
        meta.insert("schema_version", migrate::CURRENT_SCHEMA_VERSION as i64)?;
        meta.insert("project_id", Ulid::new().to_string())?;
        meta.insert("created_at", now_utc())?;

        let snapshot = doc
            .export(ExportMode::Snapshot)
            .context("failed to export initial loro snapshot")?;
        atomic_write_file(&project_dir.join(BASE_FILE), &snapshot)?;

        Ok(Self {
            root: root.to_path_buf(),
            project: project.to_string(),
            doc,
        })
    }

    pub fn open(root: &Path, project: &str) -> Result<Self> {
        validate_project_name(project)?;
        let project_dir = project_dir(root, project);
        let base_path = project_dir.join(BASE_FILE);

        if !base_path.exists() {
            bail!("project '{project}' is not initialized. Run 'td project init {project}'");
        }

        let base = fs::read(&base_path)
            .with_context(|| format!("failed to read loro snapshot '{}'", base_path.display()))?;

        let doc = LoroDoc::from_snapshot(&base).context("failed to load loro snapshot")?;
        doc.set_peer_id(load_or_create_device_peer_id(root)?)?;

        let mut deltas = collect_delta_paths(&project_dir)?;
        deltas.sort_by_key(|path| {
            path.file_stem()
                .and_then(|s| s.to_str())
                .and_then(|s| Ulid::from_string(s).ok())
        });

        for delta_path in deltas {
            let bytes = fs::read(&delta_path)
                .with_context(|| format!("failed to read loro delta '{}'", delta_path.display()))?;
            if let Err(err) = doc.import(&bytes) {
                // Tolerate malformed or stale delta files as requested by design.
                eprintln!(
                    "warning: skipping unreadable delta '{}': {err}",
                    delta_path.display()
                );
            }
        }

        // Apply any pending schema upgrades and persist the resulting delta
        // so subsequent opens don't repeat the work.
        let before_vv = doc.oplog_vv();
        let upgraded = migrate::ensure_current(&doc)?;
        if upgraded {
            doc.commit();
            let delta = doc
                .export(ExportMode::updates(&before_vv))
                .context("failed to export schema upgrade delta")?;
            let filename = format!("{}.loro", Ulid::new());
            let delta_path = project_dir.join(CHANGES_DIR).join(filename);
            atomic_write_file(&delta_path, &delta)?;
        }

        Ok(Self {
            root: root.to_path_buf(),
            project: project.to_string(),
            doc,
        })
    }

    /// Bootstrap a local project from peer-provided delta bytes.
    ///
    /// The incoming delta is imported into a fresh document, validated to
    /// ensure it carries `meta.project_id`, and then persisted as a base
    /// snapshot for future opens.
    pub fn bootstrap_from_peer(root: &Path, project: &str, delta: &[u8]) -> Result<Self> {
        validate_project_name(project)?;
        let project_dir = project_dir(root, project);
        if project_dir.exists() {
            bail!("project '{project}' already exists");
        }
        fs::create_dir_all(project_dir.join(CHANGES_DIR))?;

        let doc = LoroDoc::new();
        doc.set_peer_id(load_or_create_device_peer_id(root)?)?;
        doc.import(delta)
            .context("failed to import bootstrap delta from peer")?;
        doc.commit();

        read_project_id_from_doc(&doc)
            .context("bootstrap delta is missing required project identity")?;

        // Upgrade the peer's document before snapshotting so the local
        // copy is always at CURRENT_SCHEMA_VERSION from the start.
        migrate::ensure_current(&doc)?;
        doc.commit();

        let snapshot = doc
            .export(ExportMode::Snapshot)
            .context("failed to export bootstrap loro snapshot")?;
        atomic_write_file(&project_dir.join(BASE_FILE), &snapshot)?;

        Ok(Self {
            root: root.to_path_buf(),
            project: project.to_string(),
            doc,
        })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn project_name(&self) -> &str {
        &self.project
    }

    pub fn doc(&self) -> &LoroDoc {
        &self.doc
    }

    /// Export all current state to a fresh base snapshot.
    /// Compact accumulated deltas into the base snapshot using a two-phase
    /// protocol that is safe against concurrent writers.
    ///
    /// **Phase 1** — rename `changes/` to `changes.compacting.<ulid>/`, then
    /// immediately create a fresh `changes/`.  Any concurrent `td` command
    /// that writes a delta after this point lands in the new `changes/` and is
    /// therefore never touched by this operation.
    ///
    /// **Phase 2** — write a fresh base snapshot from the in-memory document
    /// (which was loaded from both `base.loro` and every delta at `open` time),
    /// then remove the compacting directory.
    ///
    /// Any orphaned `changes.compacting.*` directories left by a previously
    /// crashed tidy are also removed: they were already merged into `self.doc`
    /// at open time, so the new snapshot includes their contents.
    ///
    /// Returns the number of delta files folded into the snapshot.
    pub fn tidy(&self) -> Result<usize> {
        let project_dir = project_dir(&self.root, &self.project);
        let changes_dir = project_dir.join(CHANGES_DIR);

        // Phase 1: atomically hand off the current changes/ to a compacting
        // directory so new writers have a clean home immediately.
        let compacting_dir = project_dir.join(format!("changes.compacting.{}", Ulid::new()));
        if changes_dir.exists() {
            fs::rename(&changes_dir, &compacting_dir).with_context(|| {
                format!(
                    "failed to rename '{}' to '{}'",
                    changes_dir.display(),
                    compacting_dir.display()
                )
            })?;
        }
        fs::create_dir_all(&changes_dir).context("failed to create fresh changes/")?;

        // Re-import every delta from the compacting directories.  self.doc
        // was populated at open() time, but a concurrent writer may have
        // appended a delta to changes/ between open() and the Phase 1
        // rename — that delta is now inside compacting_dir without being in
        // self.doc.  CRDT import is idempotent (deduplicates by OpID), so
        // re-importing already-known ops is harmless.
        let mut compacting_deltas = collect_delta_paths(&project_dir)?;
        compacting_deltas.sort_by_key(|path| {
            path.file_stem()
                .and_then(|s| s.to_str())
                .and_then(|s| Ulid::from_string(s).ok())
        });
        for delta_path in &compacting_deltas {
            if let Ok(bytes) = fs::read(delta_path) {
                if let Err(err) = self.doc.import(&bytes) {
                    eprintln!(
                        "warning: skipping unreadable delta '{}': {err}",
                        delta_path.display()
                    );
                }
            }
        }

        // Phase 2: write the new base snapshot.  self.doc now holds the
        // full merged state including any concurrent deltas.
        let out = project_dir.join(BASE_FILE);
        let bytes = self
            .doc
            .export(ExportMode::Snapshot)
            .context("failed to export loro snapshot")?;
        atomic_write_file(&out, &bytes)?;

        // Remove the compacting directory we created in phase 1 plus any
        // orphaned changes.compacting.* dirs from previously crashed tidies.
        let mut removed = 0usize;
        for entry in fs::read_dir(&project_dir)
            .with_context(|| format!("failed to read project dir '{}'", project_dir.display()))?
        {
            let path = entry?.path();
            if !path.is_dir() {
                continue;
            }
            let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
                continue;
            };
            if !name.starts_with("changes.compacting.") {
                continue;
            }
            // Count files before removing for the summary report.
            for file in fs::read_dir(&path)
                .with_context(|| format!("failed to read '{}'", path.display()))?
            {
                let fp = file?.path();
                if fp.is_file() {
                    removed += 1;
                }
            }
            fs::remove_dir_all(&path)
                .with_context(|| format!("failed to remove compacting dir '{}'", path.display()))?;
        }

        Ok(removed)
    }

    /// Apply a local mutation and persist only the resulting delta.
    pub fn apply_and_persist<F>(&self, mutator: F) -> Result<PathBuf>
    where
        F: FnOnce(&LoroDoc) -> Result<()>,
    {
        let before = self.doc.oplog_vv();
        mutator(&self.doc)?;
        self.doc.commit();

        let delta = self
            .doc
            .export(ExportMode::updates(&before))
            .context("failed to export loro update delta")?;

        let filename = format!("{}.loro", Ulid::new());
        let path = project_dir(&self.root, &self.project)
            .join(CHANGES_DIR)
            .join(filename);
        atomic_write_file(&path, &delta)?;
        Ok(path)
    }

    /// Persist pre-built delta bytes (e.g. received from a peer) as a new
    /// change file without re-exporting from the doc.
    pub fn save_raw_delta(&self, bytes: &[u8]) -> Result<PathBuf> {
        let filename = format!("{}.loro", Ulid::new());
        let path = project_dir(&self.root, &self.project)
            .join(CHANGES_DIR)
            .join(filename);
        atomic_write_file(&path, bytes)?;
        Ok(path)
    }

    /// Return hydrated tasks, excluding tombstones.
    pub fn list_tasks(&self) -> Result<Vec<Task>> {
        self.list_tasks_inner(false)
    }

    /// Return hydrated tasks, including tombstoned rows.
    pub fn list_tasks_unfiltered(&self) -> Result<Vec<Task>> {
        self.list_tasks_inner(true)
    }

    /// Find a task by exact ULID string.
    pub fn get_task(&self, id: &TaskId, include_deleted: bool) -> Result<Option<Task>> {
        let tasks = if include_deleted {
            self.list_tasks_unfiltered()?
        } else {
            self.list_tasks()?
        };
        Ok(tasks.into_iter().find(|task| task.id == *id))
    }

    fn list_tasks_inner(&self, include_deleted: bool) -> Result<Vec<Task>> {
        let root = serde_json::to_value(self.doc.get_deep_value())?;
        let tasks_obj = root
            .get("tasks")
            .and_then(Value::as_object)
            .ok_or_else(|| anyhow!("missing root tasks map"))?;

        let mut tasks = Vec::with_capacity(tasks_obj.len());
        for (task_id_raw, task_json) in tasks_obj {
            let task = hydrate_task(task_id_raw, task_json)?;
            if include_deleted || task.deleted_at.is_none() {
                tasks.push(task);
            }
        }

        tasks.sort_by(|a, b| a.id.as_str().cmp(b.id.as_str()));
        Ok(tasks)
    }

    /// Return the stable project identity stored in `meta.project_id`.
    pub fn project_id(&self) -> Result<String> {
        read_project_id_from_doc(&self.doc)
    }

    pub fn schema_version(&self) -> Result<u32> {
        migrate::read_schema_version(&self.doc)
    }
}

/// Generate a new task ULID.
pub fn gen_id() -> TaskId {
    TaskId::new(Ulid::new())
}

pub fn parse_status(s: &str) -> Result<Status> {
    Status::parse(s)
}

pub fn parse_priority(s: &str) -> Result<Priority> {
    Priority::parse(s)
}

pub fn parse_effort(s: &str) -> Result<Effort> {
    Effort::parse(s)
}

pub fn status_label(s: Status) -> &'static str {
    s.as_str()
}

pub fn priority_label(p: Priority) -> &'static str {
    p.as_str()
}

pub fn effort_label(e: Effort) -> &'static str {
    e.as_str()
}

pub fn data_root() -> Result<PathBuf> {
    let home = std::env::var("HOME").context("HOME is not set")?;
    Ok(PathBuf::from(home).join(".local").join("share").join("td"))
}

pub fn init(cwd: &Path, project: &str) -> Result<Store> {
    let root = data_root()?;
    fs::create_dir_all(root.join(PROJECTS_DIR))?;
    let store = Store::init(&root, project)?;
    bind_project(cwd, project)?;
    Ok(store)
}

pub fn use_project(cwd: &Path, project: &str) -> Result<()> {
    let root = data_root()?;
    validate_project_name(project)?;
    if !project_dir(&root, project).join(BASE_FILE).exists() {
        bail!("project '{project}' not found. Run 'td project list' to list known projects");
    }
    bind_project(cwd, project)
}

pub fn open(start: &Path) -> Result<Store> {
    let root = data_root()?;
    let explicit = std::env::var(PROJECT_ENV).ok();
    let project = resolve_project_name(start, &root, explicit.as_deref())?.ok_or_else(|| {
        anyhow!(
            "no project selected. Use --project/TD_PROJECT, run 'td project bind <name>', or run 'td project init <name>'"
        )
    })?;
    Store::open(&root, &project)
}

/// Open the project selected by `--project`/`TD_PROJECT`/bindings if one exists.
///
/// Returns `Ok(None)` when no project is selected by any mechanism.
pub fn try_open(start: &Path) -> Result<Option<Store>> {
    let root = data_root()?;
    let explicit = std::env::var(PROJECT_ENV).ok();
    let Some(project) = resolve_project_name(start, &root, explicit.as_deref())? else {
        return Ok(None);
    };
    Store::open(&root, &project).map(Some)
}

/// Bootstrap a project from a peer delta and bind the current directory.
pub fn bootstrap_sync(cwd: &Path, project: &str, delta: &[u8]) -> Result<Store> {
    let root = data_root()?;
    fs::create_dir_all(root.join(PROJECTS_DIR))?;
    validate_project_name(project)?;
    let store = Store::bootstrap_from_peer(&root, project, delta)?;
    bind_project(cwd, project)?;
    Ok(store)
}

/// Bootstrap a project from a peer delta using an explicit data root.
///
/// Unlike [`bootstrap_sync`], this function does not consult `HOME` and is
/// therefore safe to call from async contexts where `HOME` may vary by peer.
///
/// If `bind_cwd` is true, the given working directory is bound to the new
/// project. Pass false when bootstrapping from a SyncAll context to avoid
/// unexpectedly binding directories like the user's home.
///
/// Uses exclusive file locking to prevent race conditions when multiple
/// concurrent sync operations create projects or modify bindings.
pub fn bootstrap_sync_at(
    data_root: &Path,
    cwd: &Path,
    project: &str,
    delta: &[u8],
    bind_cwd: bool,
) -> Result<Store> {
    fs::create_dir_all(data_root.join(PROJECTS_DIR))?;
    validate_project_name(project)?;

    // Exclusive lock prevents races when concurrent syncs create the same project
    // or modify bindings simultaneously.
    let lock_path = data_root.join(".bindings.lock");
    let lock_file = OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(&lock_path)
        .with_context(|| format!("failed to open lock file '{}'", lock_path.display()))?;
    lock_file
        .lock_exclusive()
        .context("failed to acquire exclusive lock on bindings")?;

    // Now holding the lock: create project and optionally update bindings atomically.
    let store = Store::bootstrap_from_peer(data_root, project, delta)?;

    if bind_cwd {
        let canonical = fs::canonicalize(cwd)
            .with_context(|| format!("failed to canonicalize '{}'", cwd.display()))?;
        let mut bindings = load_bindings(data_root)?;
        bindings
            .bindings
            .insert(canonical.to_string_lossy().to_string(), project.to_string());
        save_bindings(data_root, &bindings)?;
    }

    // Lock is released when lock_file is dropped.
    Ok(store)
}

pub fn list_projects() -> Result<Vec<String>> {
    let root = data_root()?;
    list_projects_in(&root)
}

/// List project names rooted at an explicit data directory.
///
/// Unlike [`list_projects`], this does not consult `HOME` and is therefore
/// safe to call from async contexts where `HOME` may vary between peers.
pub(crate) fn list_projects_in(root: &Path) -> Result<Vec<String>> {
    let mut out = Vec::new();
    let dir = root.join(PROJECTS_DIR);
    if !dir.exists() {
        return Ok(out);
    }

    for entry in fs::read_dir(dir)? {
        let path = entry?.path();
        if !path.is_dir() {
            continue;
        }
        let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        if path.join(BASE_FILE).exists() {
            out.push(name.to_string());
        }
    }

    out.sort();
    Ok(out)
}

pub fn resolve_task_id(store: &Store, raw: &str, include_deleted: bool) -> Result<TaskId> {
    let raw = raw.strip_prefix("td-").unwrap_or(raw);

    if let Ok(id) = TaskId::parse(raw) {
        if store.get_task(&id, include_deleted)?.is_some() {
            return Ok(id);
        }
    }

    let tasks = if include_deleted {
        store.list_tasks_unfiltered()?
    } else {
        store.list_tasks()?
    };

    let upper = raw.to_ascii_uppercase();
    let matches: Vec<TaskId> = tasks
        .into_iter()
        .filter(|t| t.id.as_str().ends_with(&upper))
        .map(|t| t.id)
        .collect();

    match matches.as_slice() {
        [] => bail!("task '{raw}' not found"),
        [id] => Ok(id.clone()),
        _ => bail!("task reference '{raw}' is ambiguous"),
    }
}

pub fn partition_blockers(store: &Store, blockers: &[TaskId]) -> Result<BlockerPartition> {
    let mut out = BlockerPartition::default();
    for blocker in blockers {
        let Some(task) = store.get_task(blocker, true)? else {
            out.resolved.push(blocker.clone());
            continue;
        };
        if task.status == Status::Closed || task.deleted_at.is_some() {
            out.resolved.push(blocker.clone());
        } else {
            out.open.push(blocker.clone());
        }
    }
    Ok(out)
}

pub fn insert_task_map(tasks: &LoroMap, task_id: &TaskId) -> Result<LoroMap> {
    tasks
        .insert_container(task_id.as_str(), LoroMap::new())
        .context("failed to create task map")
}

pub fn get_task_map(tasks: &LoroMap, task_id: &TaskId) -> Result<Option<LoroMap>> {
    match tasks.get(task_id.as_str()) {
        Some(ValueOrContainer::Container(Container::Map(map))) => Ok(Some(map)),
        Some(_) => bail!("task '{}' has invalid container type", task_id.as_str()),
        None => Ok(None),
    }
}

pub fn get_or_create_child_map(parent: &LoroMap, key: &str) -> Result<LoroMap> {
    parent
        .get_or_create_container(key, LoroMap::new())
        .with_context(|| format!("failed to get or create map key '{key}'"))
}

fn bindings_path(root: &Path) -> PathBuf {
    root.join(BINDINGS_FILE)
}

fn resolve_project_name(
    start: &Path,
    root: &Path,
    explicit: Option<&str>,
) -> Result<Option<String>> {
    if let Some(project) = explicit {
        validate_project_name(project)?;
        return Ok(Some(project.to_string()));
    }

    let cwd = canonicalize_binding_path(start)?;
    let bindings = load_bindings(root)?;

    let mut best: Option<(usize, String)> = None;
    for (raw_path, project) in bindings.bindings {
        let bound = PathBuf::from(raw_path);
        if is_prefix_path(&bound, &cwd) {
            let score = bound.components().count();
            match &best {
                Some((best_score, _)) if *best_score >= score => {}
                _ => best = Some((score, project)),
            }
        }
    }

    if let Some((_, project)) = best {
        return Ok(Some(project));
    }

    Ok(None)
}

pub fn unbind_project(cwd: &Path) -> Result<()> {
    let root = data_root()?;
    let canonical = canonicalize_binding_path(cwd)?;
    let canonical_str = canonical.to_string_lossy().to_string();

    let mut bindings = load_bindings(&root)?;
    if !bindings.bindings.contains_key(&canonical_str) {
        bail!("path '{}' is not bound to any project", canonical.display());
    }
    bindings.bindings.remove(&canonical_str);
    save_bindings(&root, &bindings)
}

pub fn delete_project(name: &str) -> Result<()> {
    validate_project_name(name)?;
    let root = data_root()?;
    let proj_dir = project_dir(&root, name);

    if !proj_dir.join(BASE_FILE).exists() {
        bail!("project '{name}' not found");
    }

    fs::remove_dir_all(&proj_dir).with_context(|| {
        format!(
            "failed to remove project directory '{}'",
            proj_dir.display()
        )
    })?;

    let mut bindings = load_bindings(&root)?;
    bindings.bindings.retain(|_, project| project != name);
    save_bindings(&root, &bindings)
}

fn bind_project(cwd: &Path, project: &str) -> Result<()> {
    validate_project_name(project)?;

    let root = data_root()?;
    fs::create_dir_all(&root)?;

    let canonical = canonicalize_binding_path(cwd)?;
    let mut bindings = load_bindings(&root)?;
    bindings
        .bindings
        .insert(canonical.to_string_lossy().to_string(), project.to_string());
    save_bindings(&root, &bindings)
}

fn load_bindings(root: &Path) -> Result<BindingsFile> {
    let path = bindings_path(root);
    if !path.exists() {
        return Ok(BindingsFile::default());
    }
    let content = fs::read_to_string(&path)
        .with_context(|| format!("failed reading bindings from '{}'", path.display()))?;
    serde_json::from_str(&content)
        .with_context(|| format!("invalid bindings file '{}'", path.display()))
}

fn save_bindings(root: &Path, bindings: &BindingsFile) -> Result<()> {
    let path = bindings_path(root);
    let bytes = serde_json::to_vec_pretty(bindings)?;
    atomic_write_file(&path, &bytes)
}

fn canonicalize_binding_path(path: &Path) -> Result<PathBuf> {
    fs::canonicalize(path).with_context(|| format!("failed to canonicalize '{}'", path.display()))
}

fn is_prefix_path(prefix: &Path, target: &Path) -> bool {
    let mut prefix_components = prefix.components();
    let mut target_components = target.components();

    loop {
        match (prefix_components.next(), target_components.next()) {
            (None, _) => return true,
            (Some(_), None) => return false,
            (Some(a), Some(b)) if a == b => continue,
            _ => return false,
        }
    }
}

pub fn validate_project_name(name: &str) -> Result<()> {
    if name.is_empty() {
        bail!("project name cannot be empty");
    }
    if name.contains('/') || name.contains('\\') || name == "." || name == ".." {
        bail!("invalid project name '{name}'");
    }
    if name.chars().any(char::is_control) {
        bail!("invalid project name '{name}'");
    }
    Ok(())
}

fn read_project_id_from_doc(doc: &LoroDoc) -> Result<String> {
    let root = serde_json::to_value(doc.get_deep_value())?;
    root.get("meta")
        .and_then(|m| m.get("project_id"))
        .and_then(Value::as_str)
        .map(str::to_owned)
        .ok_or_else(|| anyhow!("missing meta.project_id in project doc"))
}

fn hydrate_task(task_id_raw: &str, value: &Value) -> Result<Task> {
    let obj = value
        .as_object()
        .ok_or_else(|| anyhow!("task '{task_id_raw}' is not an object"))?;

    let id = TaskId::parse(task_id_raw)?;

    let title = get_required_string(obj, "title")?;
    let description = get_required_string(obj, "description")?;
    let task_type = get_required_string(obj, "type")?;
    let status = Status::parse(&get_required_string(obj, "status")?)?;
    let priority = Priority::parse(&get_required_string(obj, "priority")?)?;
    let effort = Effort::parse(&get_required_string(obj, "effort")?)?;
    let parent = match obj.get("parent").and_then(Value::as_str) {
        Some("") | None => None,
        Some(raw) => Some(TaskId::parse(raw)?),
    };

    let created_at = get_required_string(obj, "created_at")?;
    let updated_at = get_required_string(obj, "updated_at")?;
    let deleted_at = obj
        .get("deleted_at")
        .and_then(Value::as_str)
        .map(str::to_owned)
        .filter(|s| !s.is_empty());

    let labels = obj
        .get("labels")
        .and_then(Value::as_object)
        .map(|m| m.keys().cloned().collect())
        .unwrap_or_default();

    let blockers = obj
        .get("blockers")
        .and_then(Value::as_object)
        .map(|m| {
            m.keys()
                .map(|raw| TaskId::parse(raw))
                .collect::<Result<Vec<_>>>()
        })
        .transpose()?
        .unwrap_or_default();

    let mut logs = obj
        .get("logs")
        .and_then(Value::as_object)
        .map(|logs| {
            logs.iter()
                .map(|(log_id_raw, payload)| {
                    let payload_obj = payload.as_object().ok_or_else(|| {
                        anyhow!("log '{log_id_raw}' on task '{task_id_raw}' is not an object")
                    })?;
                    Ok(LogEntry {
                        id: TaskId::parse(log_id_raw)?,
                        timestamp: get_required_string(payload_obj, "timestamp")?,
                        message: get_required_string(payload_obj, "message")?,
                    })
                })
                .collect::<Result<Vec<_>>>()
        })
        .transpose()?
        .unwrap_or_default();

    logs.sort_by(|a, b| a.id.as_str().cmp(b.id.as_str()));

    Ok(Task {
        id,
        title,
        description,
        task_type,
        priority,
        status,
        effort,
        parent,
        created_at,
        updated_at,
        deleted_at,
        labels,
        blockers,
        logs,
    })
}

fn get_required_string(map: &serde_json::Map<String, Value>, key: &str) -> Result<String> {
    map.get(key)
        .and_then(Value::as_str)
        .map(str::to_owned)
        .ok_or_else(|| anyhow!("missing or non-string key '{key}'"))
}

fn collect_delta_paths(project_dir: &Path) -> Result<Vec<PathBuf>> {
    let mut paths = Vec::new();
    collect_changes_from_dir(&project_dir.join(CHANGES_DIR), &mut paths)?;

    for entry in fs::read_dir(project_dir)? {
        let path = entry?.path();
        if !path.is_dir() {
            continue;
        }
        let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        if name.starts_with("changes.compacting.") {
            collect_changes_from_dir(&path, &mut paths)?;
        }
    }

    Ok(paths)
}

fn collect_changes_from_dir(dir: &Path, out: &mut Vec<PathBuf>) -> Result<()> {
    if !dir.exists() {
        return Ok(());
    }

    for entry in fs::read_dir(dir)? {
        let path = entry?.path();
        if !path.is_file() {
            continue;
        }

        let Some(filename) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        if filename.ends_with(TMP_SUFFIX) || !filename.ends_with(".loro") {
            continue;
        }

        let Some(stem) = path.file_stem().and_then(|s| s.to_str()) else {
            continue;
        };
        if Ulid::from_string(stem).is_err() {
            continue;
        }

        out.push(path);
    }

    Ok(())
}

fn project_dir(root: &Path, project: &str) -> PathBuf {
    root.join(PROJECTS_DIR).join(project)
}

fn load_or_create_device_peer_id(root: &Path) -> Result<PeerID> {
    let path = root.join("device_id");
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }

    let device_ulid = if path.exists() {
        let content = fs::read_to_string(&path)
            .with_context(|| format!("failed reading device id from '{}'", path.display()))?;
        Ulid::from_string(content.trim()).context("invalid persisted device id ULID")?
    } else {
        let id = Ulid::new();
        atomic_write_file(&path, id.to_string().as_bytes())?;
        id
    };

    let raw: u128 = device_ulid.into();
    Ok((raw & u64::MAX as u128) as u64)
}

fn atomic_write_file(path: &Path, bytes: &[u8]) -> Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| anyhow!("cannot atomically write root path '{}'", path.display()))?;
    fs::create_dir_all(parent)?;

    let tmp_name = format!(
        "{}.{}{}",
        path.file_name().and_then(|n| n.to_str()).unwrap_or("write"),
        Ulid::new(),
        TMP_SUFFIX
    );
    let tmp_path = parent.join(tmp_name);

    {
        let mut file = OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&tmp_path)
            .with_context(|| format!("failed to open temp file '{}'", tmp_path.display()))?;
        file.write_all(bytes)?;
        file.sync_all()?;
    }

    fs::rename(&tmp_path, path).with_context(|| {
        format!(
            "failed to atomically rename '{}' to '{}'",
            tmp_path.display(),
            path.display()
        )
    })?;

    sync_dir(parent)?;
    Ok(())
}

fn sync_dir(path: &Path) -> Result<()> {
    let dir =
        File::open(path).with_context(|| format!("failed opening dir '{}'", path.display()))?;
    dir.sync_all()
        .with_context(|| format!("failed fsync on dir '{}'", path.display()))?;
    Ok(())
}
