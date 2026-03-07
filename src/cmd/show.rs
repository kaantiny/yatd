use anyhow::Result;
use std::path::Path;

use crate::db;

pub fn run(root: &Path, id: &str, json: bool) -> Result<()> {
    let store = db::open(root)?;
    let task_id = db::resolve_task_id(&store, id, true)?;
    let task = store
        .get_task(&task_id, true)?
        .ok_or_else(|| anyhow::anyhow!("task {id} not found"))?;

    if json {
        println!("{}", serde_json::to_string(&task)?);
        return Ok(());
    }

    let c = crate::color::stdout_theme();

    println!(
        "{}# {}{} {}[{}]{}",
        c.bold,
        task.title,
        c.reset,
        c.yellow,
        db::status_label(task.status),
        c.reset
    );

    if !task.description.is_empty() {
        println!();
        println!("{}", task.description);
    }

    println!();
    println!(
        "{}{}{} · {} · {}{}{} priority · {}{}{} effort",
        c.bold,
        task.id,
        c.reset,
        task.task_type,
        c.red,
        db::priority_label(task.priority),
        c.reset,
        c.blue,
        db::effort_label(task.effort),
        c.reset,
    );

    if !task.labels.is_empty() {
        println!("labels: {}", task.labels.join(", "));
    }

    let blockers = db::partition_blockers(&store, &task.blockers)?;
    let total = blockers.open.len() + blockers.resolved.len();
    if total > 0 {
        let label = if total == 1 { "blocker" } else { "blockers" };
        if blockers.open.is_empty() {
            // All closed: prefix with [all closed], no individual markers.
            let ids: Vec<String> = blockers.resolved.iter().map(ToString::to_string).collect();
            println!("{label}: [all closed] {}", ids.join(", "));
        } else {
            // Mixed or all open: annotate only the closed ones.
            let mut ids: Vec<String> = blockers.open.iter().map(ToString::to_string).collect();
            ids.extend(blockers.resolved.iter().map(|id| format!("{id} [closed]")));
            println!("{label}: {}", ids.join(", "));
        }
    }

    println!("created {} · updated {}", task.created_at, task.updated_at);

    if !task.logs.is_empty() {
        println!();
        println!("--- log ---");
        for log in task.logs {
            println!("[{}] {}", log.timestamp, log.message);
        }
    }

    Ok(())
}
