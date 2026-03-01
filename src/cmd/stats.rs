use anyhow::Result;
use std::path::Path;

use crate::db;

pub fn run(root: &Path) -> Result<()> {
    let store = db::open(root)?;
    let tasks = store.list_tasks()?;

    let total = tasks.len();
    let open = tasks
        .iter()
        .filter(|t| t.status == db::Status::Open)
        .count();
    let in_progress = tasks
        .iter()
        .filter(|t| t.status == db::Status::InProgress)
        .count();
    let closed = tasks
        .iter()
        .filter(|t| t.status == db::Status::Closed)
        .count();

    println!(
        "{}",
        serde_json::json!({
            "total": total,
            "open": open,
            "in_progress": in_progress,
            "closed": closed,
        })
    );

    Ok(())
}
