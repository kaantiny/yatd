use anyhow::Result;
use std::path::Path;

use crate::db;

pub fn run(root: &Path, ids: &[String], json: bool) -> Result<()> {
    let store = db::open(root)?;
    let ts = db::now_utc();

    let mut reopened = Vec::new();
    for raw in ids {
        let id = db::resolve_task_id(&store, raw, false)?;
        store.apply_and_persist(|doc| {
            let tasks = doc.get_map("tasks");
            if let Some(task) = db::get_task_map(&tasks, &id)? {
                task.insert("status", db::status_label(db::Status::Open))?;
                task.insert("updated_at", ts.clone())?;
            }
            Ok(())
        })?;
        reopened.push(id);
    }

    if json {
        let out: Vec<_> = reopened
            .iter()
            .map(|id| serde_json::json!({"id": id, "status": "open"}))
            .collect();
        println!("{}", serde_json::to_string(&out)?);
    } else {
        let c = crate::color::stdout_theme();
        for id in &reopened {
            println!("{}reopened{} {id}", c.green, c.reset);
        }
    }

    Ok(())
}
