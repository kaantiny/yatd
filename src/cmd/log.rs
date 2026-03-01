use anyhow::Result;
use loro::LoroMap;
use std::path::Path;

use crate::db;

pub fn run(root: &Path, id: &str, message: &str, json: bool) -> Result<()> {
    let store = db::open(root)?;
    let task_id = db::resolve_task_id(&store, id, false)?;
    let log_id = db::gen_id();
    let ts = db::now_utc();

    store.apply_and_persist(|doc| {
        let tasks = doc.get_map("tasks");
        let task =
            db::get_task_map(&tasks, &task_id)?.ok_or_else(|| anyhow::anyhow!("task not found"))?;
        let logs = db::get_or_create_child_map(&task, "logs")?;
        let entry = logs.insert_container(log_id.as_str(), LoroMap::new())?;
        entry.insert("timestamp", ts.clone())?;
        entry.insert("message", message)?;
        task.insert("updated_at", ts.clone())?;
        Ok(())
    })?;

    let entry = db::LogEntry {
        id: log_id,
        timestamp: ts,
        message: message.to_string(),
    };

    if json {
        println!("{}", serde_json::to_string(&entry)?);
    } else {
        println!("logged to {}", task_id);
    }

    Ok(())
}
