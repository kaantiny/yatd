use anyhow::{bail, Result};
use std::path::Path;

use crate::db;

pub fn run(root: &Path, id: &str, json: bool) -> Result<()> {
    let conn = db::open(root)?;

    let exists: bool = conn.query_row("SELECT COUNT(*) FROM tasks WHERE id = ?1", [id], |r| {
        r.get::<_, i64>(0).map(|n| n > 0)
    })?;

    if !exists {
        bail!("task {id} not found");
    }

    let detail = db::load_task_detail(&conn, id)?;

    if json {
        println!("{}", serde_json::to_string(&detail)?);
    } else {
        let c = crate::color::stdout_theme();
        let t = &detail.task;
        println!("{}          id{} = {}", c.bold, c.reset, t.id);
        println!("{}       title{} = {}", c.bold, c.reset, t.title);
        println!("{}      status{} = {}", c.bold, c.reset, t.status);
        println!("{}    priority{} = {}", c.bold, c.reset, t.priority);
        println!("{}      effort{} = {}", c.bold, c.reset, t.effort);
        println!("{}        type{} = {}", c.bold, c.reset, t.task_type);
        if !t.description.is_empty() {
            println!("{} description{} = {}", c.bold, c.reset, t.description);
        }
        println!("{}     created{} = {}", c.bold, c.reset, t.created);
        println!("{}     updated{} = {}", c.bold, c.reset, t.updated);
        if !detail.labels.is_empty() {
            println!(
                "{}      labels{} = {}",
                c.bold,
                c.reset,
                detail.labels.join(",")
            );
        }
        if !detail.blockers.is_empty() {
            println!(
                "{}    blockers{} = {}",
                c.bold,
                c.reset,
                detail.blockers.join(",")
            );
        }
    }

    Ok(())
}
