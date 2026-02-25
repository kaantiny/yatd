use anyhow::Result;
use std::path::Path;

use crate::db;

pub fn run(root: &Path, json: bool) -> Result<()> {
    let conn = db::open(root)?;

    let mut stmt = conn.prepare(
        "SELECT id, title, description, type, priority, status, effort, parent, created, updated
         FROM tasks
         WHERE status = 'open'
           AND id NOT IN (
               SELECT b.task_id FROM blockers b
               JOIN tasks t ON b.blocker_id = t.id
               WHERE t.status != 'closed'
           )
         ORDER BY priority, created",
    )?;

    let tasks: Vec<db::Task> = stmt
        .query_map([], db::row_to_task)?
        .collect::<rusqlite::Result<_>>()?;

    if json {
        let summary: Vec<serde_json::Value> = tasks
            .iter()
            .map(|t| {
                serde_json::json!({
                    "id": t.id,
                    "title": t.title,
                    "priority": db::priority_label(t.priority),
                    "effort": db::effort_label(t.effort),
                })
            })
            .collect();
        println!("{}", serde_json::to_string(&summary)?);
    } else {
        let c = crate::color::stdout_theme();
        for t in &tasks {
            println!(
                "{}{:<12}{} {}{:<8}{} {}",
                c.green,
                t.id,
                c.reset,
                c.red,
                db::priority_label(t.priority),
                c.reset,
                t.title
            );
        }
    }

    Ok(())
}
