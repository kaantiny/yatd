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
        return Ok(());
    }

    let c = crate::color::stdout_theme();
    let t = &detail.task;

    // Title as a heading with status tag
    println!(
        "{}# {}{} {}[{}]{}",
        c.bold, t.title, c.reset, c.yellow, t.status, c.reset
    );

    // Description as body text, only when present
    if !t.description.is_empty() {
        println!();
        println!("{}", t.description);
    }

    // Metadata line: id · type · priority · effort
    println!();
    println!(
        "{}{}{} · {} · {}{}{} priority · {}{}{} effort",
        c.bold,
        t.id,
        c.reset,
        t.task_type,
        c.red,
        db::priority_label(t.priority),
        c.reset,
        c.blue,
        db::effort_label(t.effort),
        c.reset,
    );

    // Labels, only when present
    if !detail.labels.is_empty() {
        println!("labels: {}", detail.labels.join(", "));
    }

    // Blockers, only when present
    let (open_blockers, closed_blockers) = db::load_blockers_partitioned(&conn, &t.id)?;
    let total = open_blockers.len() + closed_blockers.len();
    if total > 0 {
        let label = if total == 1 { "blocker" } else { "blockers" };
        let mut ids: Vec<String> = Vec::new();
        for id in &open_blockers {
            ids.push(id.clone());
        }
        for id in &closed_blockers {
            ids.push(format!("{id} [closed]"));
        }

        let value = if open_blockers.is_empty() {
            format!("[all closed] {}", ids.join(", "))
        } else {
            ids.join(", ")
        };

        println!("{label}: {value}");
    }

    // Timestamps at the bottom
    println!("created {} · updated {}", t.created, t.updated);

    Ok(())
}
