use anyhow::{anyhow, Result};
use std::collections::BTreeSet;
use std::path::Path;

use crate::cli::LabelAction;
use crate::db;

pub fn run(root: &Path, action: &LabelAction, json: bool) -> Result<()> {
    let store = db::open(root)?;

    match action {
        LabelAction::Add { id, label } => {
            let task_id = db::resolve_task_id(&store, id, false)?;
            let ts = db::now_utc();
            store.apply_and_persist(|doc| {
                let tasks = doc.get_map("tasks");
                let task =
                    db::get_task_map(&tasks, &task_id)?.ok_or_else(|| anyhow!("task not found"))?;
                let labels = db::get_or_create_child_map(&task, "labels")?;
                labels.insert(label, true)?;
                task.insert("updated_at", ts.clone())?;
                Ok(())
            })?;

            if json {
                println!(
                    "{}",
                    serde_json::json!({"id": task_id.as_str(), "label": label})
                );
            } else {
                let c = crate::color::stdout_theme();
                println!("{}added{} label {label}", c.green, c.reset);
            }
        }
        LabelAction::Rm { id, label } => {
            let task_id = db::resolve_task_id(&store, id, false)?;
            let ts = db::now_utc();
            store.apply_and_persist(|doc| {
                let tasks = doc.get_map("tasks");
                let task =
                    db::get_task_map(&tasks, &task_id)?.ok_or_else(|| anyhow!("task not found"))?;
                let labels = db::get_or_create_child_map(&task, "labels")?;
                labels.delete(label)?;
                task.insert("updated_at", ts.clone())?;
                Ok(())
            })?;

            if !json {
                let c = crate::color::stdout_theme();
                println!("{}removed{} label {label}", c.green, c.reset);
            }
        }
        LabelAction::List { id } => {
            let task_id = db::resolve_task_id(&store, id, false)?;
            let task = store
                .get_task(&task_id, false)?
                .ok_or_else(|| anyhow!("task not found"))?;
            if json {
                println!("{}", serde_json::to_string(&task.labels)?);
            } else {
                for l in &task.labels {
                    println!("{l}");
                }
            }
        }
        LabelAction::ListAll => {
            let mut set = BTreeSet::new();
            for task in store.list_tasks()? {
                for label in task.labels {
                    set.insert(label);
                }
            }
            let labels: Vec<_> = set.into_iter().collect();
            if json {
                println!("{}", serde_json::to_string(&labels)?);
            } else {
                for l in &labels {
                    println!("{l}");
                }
            }
        }
    }

    Ok(())
}
