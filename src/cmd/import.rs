use anyhow::{anyhow, Result};
use loro::LoroMap;
use serde::Deserialize;
use std::io::BufRead;
use std::path::Path;

use crate::db;

#[derive(Deserialize)]
struct ImportTask {
    id: String,
    title: String,
    #[serde(default)]
    description: String,
    #[serde(rename = "type", default = "default_type")]
    task_type: String,
    #[serde(default = "default_priority")]
    priority: String,
    #[serde(default = "default_status")]
    status: String,
    #[serde(default = "default_effort")]
    effort: String,
    #[serde(default)]
    parent: Option<String>,
    created_at: String,
    updated_at: String,
    #[serde(default)]
    deleted_at: Option<String>,
    #[serde(default)]
    labels: Vec<String>,
    #[serde(default)]
    blockers: Vec<String>,
    #[serde(default)]
    logs: Vec<ImportLog>,
}

#[derive(Deserialize)]
struct ImportLog {
    id: String,
    timestamp: String,
    message: String,
}

fn default_type() -> String {
    "task".into()
}
fn default_priority() -> String {
    "medium".into()
}
fn default_status() -> String {
    "open".into()
}
fn default_effort() -> String {
    "medium".into()
}

pub fn run(root: &Path, file: &str) -> Result<()> {
    let store = db::open(root)?;

    let reader: Box<dyn BufRead> = if file == "-" {
        Box::new(std::io::stdin().lock())
    } else {
        Box::new(std::io::BufReader::new(std::fs::File::open(file)?))
    };

    for line in reader.lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        let t: ImportTask = serde_json::from_str(&line)?;
        let id = db::TaskId::parse(&t.id)?;
        db::parse_priority(&t.priority)?;
        db::parse_status(&t.status)?;
        db::parse_effort(&t.effort)?;

        store.apply_and_persist(|doc| {
            let tasks = doc.get_map("tasks");
            let task = if let Some(existing) = db::get_task_map(&tasks, &id)? {
                existing
            } else {
                db::insert_task_map(&tasks, &id)?
            };

            task.insert("title", t.title.clone())?;
            task.insert("description", t.description.clone())?;
            task.insert("type", t.task_type.clone())?;
            task.insert("priority", t.priority.clone())?;
            task.insert("status", t.status.clone())?;
            task.insert("effort", t.effort.clone())?;
            task.insert("parent", t.parent.as_deref().unwrap_or(""))?;
            task.insert("created_at", t.created_at.clone())?;
            task.insert("updated_at", t.updated_at.clone())?;
            task.insert("deleted_at", t.deleted_at.as_deref().unwrap_or(""))?;

            let labels = task.insert_container("labels", LoroMap::new())?;
            for lbl in &t.labels {
                labels.insert(lbl, true)?;
            }
            let blockers = task.insert_container("blockers", LoroMap::new())?;
            for blk in &t.blockers {
                let parsed =
                    db::TaskId::parse(blk).map_err(|_| anyhow!("invalid blocker id '{blk}'"))?;
                blockers.insert(parsed.as_str(), true)?;
            }
            let logs = task.insert_container("logs", LoroMap::new())?;
            for entry in &t.logs {
                let log_id = db::TaskId::parse(&entry.id)
                    .map_err(|_| anyhow!("invalid log id '{}'", entry.id))?;
                let record = logs.insert_container(log_id.as_str(), LoroMap::new())?;
                record.insert("timestamp", entry.timestamp.clone())?;
                record.insert("message", entry.message.clone())?;
            }
            Ok(())
        })?;
    }

    Ok(())
}
