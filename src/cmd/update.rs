use anyhow::{anyhow, Result};
use std::path::Path;

use crate::db;

pub struct Opts<'a> {
    pub status: Option<&'a str>,
    pub priority: Option<db::Priority>,
    pub effort: Option<db::Effort>,
    pub title: Option<&'a str>,
    pub desc: Option<&'a str>,
    pub json: bool,
}

pub fn run(root: &Path, id: &str, opts: Opts) -> Result<()> {
    let store = db::open(root)?;
    let task_id = db::resolve_task_id(&store, id, false)?;
    let ts = db::now_utc();

    let parsed_status = opts.status.map(db::parse_status).transpose()?;

    store.apply_and_persist(|doc| {
        let tasks = doc.get_map("tasks");
        let task = db::get_task_map(&tasks, &task_id)?.ok_or_else(|| anyhow!("task not found"))?;

        if let Some(s) = parsed_status {
            task.insert("status", db::status_label(s))?;
        }
        if let Some(p) = opts.priority {
            task.insert("priority", db::priority_label(p))?;
        }
        if let Some(e) = opts.effort {
            task.insert("effort", db::effort_label(e))?;
        }
        if let Some(t) = opts.title {
            task.insert("title", t)?;
        }
        if let Some(d) = opts.desc {
            task.insert("description", d)?;
        }
        task.insert("updated_at", ts.clone())?;
        Ok(())
    })?;

    if opts.json {
        let task = store
            .get_task(&task_id, false)?
            .ok_or_else(|| anyhow!("task not found"))?;
        println!("{}", serde_json::to_string(&task)?);
    } else {
        let c = crate::color::stdout_theme();
        println!("{}updated{} {}", c.green, c.reset, task_id);
    }

    Ok(())
}
