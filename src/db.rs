use anyhow::{anyhow, bail, Context, Result};
use loro::{ExportMode, LoroDoc, PeerID};
use serde::Serialize;
use serde_json::Value;
use std::fmt;
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use ulid::Ulid;

const TD_DIR: &str = ".td";
const PROJECTS_DIR: &str = "projects";
const CHANGES_DIR: &str = "changes";
const BASE_FILE: &str = "base.loro";
const TMP_SUFFIX: &str = ".tmp";
const SCHEMA_VERSION: u32 = 1;

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
}

/// A stable task identifier backed by a ULID.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
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
        self.0.chars().take(7).collect()
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

/// Storage wrapper around one project's Loro document and disk layout.
#[derive(Debug, Clone)]
pub struct Store {
    root: PathBuf,
    project: String,
    doc: LoroDoc,
}

impl Store {
    /// Create a new store rooted at the current project path.
    pub fn init(root: &Path) -> Result<Self> {
        let project = project_name(root)?;
        let project_dir = project_dir(root, &project);
        fs::create_dir_all(project_dir.join(CHANGES_DIR))?;

        let doc = LoroDoc::new();
        let peer_id = load_or_create_device_peer_id()?;
        doc.set_peer_id(peer_id)?;

        doc.get_map("tasks");
        let meta = doc.get_map("meta");
        meta.insert("schema_version", SCHEMA_VERSION as i64)?;
        meta.insert("project_id", Ulid::new().to_string())?;
        meta.insert("created_at", now_utc())?;

        let snapshot = doc
            .export(ExportMode::Snapshot)
            .context("failed to export initial loro snapshot")?;
        atomic_write_file(&project_dir.join(BASE_FILE), &snapshot)?;

        Ok(Self {
            root: root.to_path_buf(),
            project,
            doc,
        })
    }

    /// Open an existing store and replay deltas.
    pub fn open(root: &Path) -> Result<Self> {
        let project = project_name(root)?;
        let project_dir = project_dir(root, &project);
        let base_path = project_dir.join(BASE_FILE);

        if !base_path.exists() {
            bail!("not initialized. Run 'td init'");
        }

        let base = fs::read(&base_path)
            .with_context(|| format!("failed to read loro snapshot '{}'", base_path.display()))?;

        let doc = LoroDoc::from_snapshot(&base).context("failed to load loro snapshot")?;
        doc.set_peer_id(load_or_create_device_peer_id()?)?;

        let mut deltas = collect_delta_paths(&project_dir)?;
        deltas.sort_by_key(|path| {
            path.file_stem()
                .and_then(|s| s.to_str())
                .and_then(|s| Ulid::from_string(s).ok())
        });

        for delta_path in deltas {
            let bytes = fs::read(&delta_path)
                .with_context(|| format!("failed to read loro delta '{}'", delta_path.display()))?;
            doc.import(&bytes).with_context(|| {
                format!("failed to import loro delta '{}'", delta_path.display())
            })?;
        }

        Ok(Self {
            root: root.to_path_buf(),
            project,
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
    pub fn write_snapshot(&self) -> Result<PathBuf> {
        let out = project_dir(&self.root, &self.project).join(BASE_FILE);
        let bytes = self
            .doc
            .export(ExportMode::Snapshot)
            .context("failed to export loro snapshot")?;
        atomic_write_file(&out, &bytes)?;
        Ok(out)
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

    /// Return current schema version from root meta map.
    pub fn schema_version(&self) -> Result<u32> {
        let root = serde_json::to_value(self.doc.get_deep_value())?;
        let meta = root
            .get("meta")
            .and_then(Value::as_object)
            .ok_or_else(|| anyhow!("missing root meta map"))?;
        let n = meta
            .get("schema_version")
            .and_then(Value::as_u64)
            .ok_or_else(|| anyhow!("invalid or missing meta.schema_version"))?;
        Ok(n as u32)
    }
}

/// Generate a new task ULID.
pub fn gen_id() -> TaskId {
    TaskId::new(Ulid::new())
}

/// Parse a priority string value.
pub fn parse_priority(s: &str) -> Result<Priority> {
    Priority::parse(s)
}

/// Parse an effort string value.
pub fn parse_effort(s: &str) -> Result<Effort> {
    Effort::parse(s)
}

/// Convert a priority value to its storage label.
pub fn priority_label(p: Priority) -> &'static str {
    p.as_str()
}

/// Convert an effort value to its storage label.
pub fn effort_label(e: Effort) -> &'static str {
    e.as_str()
}

/// Walk up from `start` looking for a `.td/` directory.
pub fn find_root(start: &Path) -> Result<PathBuf> {
    let mut dir = start.to_path_buf();
    loop {
        if dir.join(TD_DIR).is_dir() {
            return Ok(dir);
        }
        if !dir.pop() {
            bail!("not initialized. Run 'td init'");
        }
    }
}

/// Return the path to the `.td/` directory under `root`.
pub fn td_dir(root: &Path) -> PathBuf {
    root.join(TD_DIR)
}

/// Initialize on-disk project storage and return the opened store.
pub fn init(root: &Path) -> Result<Store> {
    fs::create_dir_all(td_dir(root))?;
    Store::init(root)
}

/// Open an existing project's storage.
pub fn open(root: &Path) -> Result<Store> {
    Store::open(root)
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
        .unwrap_or_else(Vec::new);

    let blockers = obj
        .get("blockers")
        .and_then(Value::as_object)
        .map(|m| {
            m.keys()
                .map(|raw| TaskId::parse(raw))
                .collect::<Result<Vec<_>>>()
        })
        .transpose()?
        .unwrap_or_else(Vec::new);

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
        .unwrap_or_else(Vec::new);

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
        let entry = entry?;
        let path = entry.path();
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
        let entry = entry?;
        let path = entry.path();
        if !path.is_file() {
            continue;
        }

        let Some(filename) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        if filename.ends_with(TMP_SUFFIX) {
            continue;
        }
        if !filename.ends_with(".loro") {
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

fn project_name(root: &Path) -> Result<String> {
    root.file_name()
        .and_then(|n| n.to_str())
        .map(str::to_owned)
        .ok_or_else(|| {
            anyhow!(
                "could not infer project name from path '{}'",
                root.display()
            )
        })
}

fn project_dir(root: &Path, project: &str) -> PathBuf {
    td_dir(root).join(PROJECTS_DIR).join(project)
}

fn load_or_create_device_peer_id() -> Result<PeerID> {
    let home = std::env::var("HOME").context("HOME is not set")?;
    let path = PathBuf::from(home)
        .join(".local")
        .join("share")
        .join("td")
        .join("device_id");

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

    Ok((device_ulid.to_u128() & u64::MAX as u128) as u64)
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
