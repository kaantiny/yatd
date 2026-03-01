use anyhow::{anyhow, Result};
use loro::LoroMap;
use std::path::Path;

use crate::db;

pub struct Opts<'a> {
    pub title: Option<&'a str>,
    pub priority: db::Priority,
    pub effort: db::Effort,
    pub task_type: &'a str,
    pub desc: Option<&'a str>,
    pub parent: Option<&'a str>,
    pub labels: Option<&'a str>,
    pub json: bool,
}

pub fn run(root: &Path, opts: Opts) -> Result<()> {
    let title = opts.title.ok_or_else(|| anyhow!("title required"))?;
    let desc = opts.desc.unwrap_or("");
    let ts = db::now_utc();

    let store = db::open(root)?;
    let id = db::gen_id();

    let parent = if let Some(raw) = opts.parent {
        Some(db::resolve_task_id(&store, raw, false)?)
    } else {
        None
    };

    store.apply_and_persist(|doc| {
        let tasks = doc.get_map("tasks");
        let task = db::insert_task_map(&tasks, &id)?;

        task.insert("title", title)?;
        task.insert("description", desc)?;
        task.insert("type", opts.task_type)?;
        task.insert("priority", db::priority_label(opts.priority))?;
        task.insert("status", db::status_label(db::Status::Open))?;
        task.insert("effort", db::effort_label(opts.effort))?;
        task.insert("parent", parent.as_ref().map(|p| p.as_str()).unwrap_or(""))?;
        task.insert("created_at", ts.clone())?;
        task.insert("updated_at", ts.clone())?;
        task.insert("deleted_at", "")?;
        task.insert_container("labels", LoroMap::new())?;
        task.insert_container("blockers", LoroMap::new())?;
        task.insert_container("logs", LoroMap::new())?;

        if let Some(label_str) = opts.labels {
            let labels = db::get_or_create_child_map(&task, "labels")?;
            for lbl in label_str
                .split(',')
                .map(str::trim)
                .filter(|l| !l.is_empty())
            {
                labels.insert(lbl, true)?;
            }
        }

        Ok(())
    })?;

    let task = store
        .get_task(&id, false)?
        .ok_or_else(|| anyhow!("failed to reload created task"))?;

    if opts.json {
        println!("{}", serde_json::to_string(&task)?);
    } else {
        let c = crate::color::stdout_theme();
        println!("{}created{} {}: {}", c.green, c.reset, task.id, task.title);
    }

    Ok(())
}
