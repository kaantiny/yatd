use anyhow::Result;
use comfy_table::presets::NOTHING;
use comfy_table::Table;
use std::path::Path;

use crate::db;

pub fn run(root: &Path, query: &str, json: bool) -> Result<()> {
    let conn = db::open(root)?;
    let pattern = format!("%{query}%");

    let mut stmt = conn.prepare(
        "SELECT id, title, description, type, priority, status, effort, parent, created, updated
         FROM tasks
         WHERE title LIKE ?1 OR description LIKE ?1",
    )?;

    let tasks: Vec<db::Task> = stmt
        .query_map([&pattern], db::row_to_task)?
        .collect::<rusqlite::Result<_>>()?;

    if json {
        let summary: Vec<serde_json::Value> = tasks
            .iter()
            .map(|t| {
                serde_json::json!({
                    "id": t.id,
                    "title": t.title,
                    "status": t.status,
                })
            })
            .collect();
        println!("{}", serde_json::to_string(&summary)?);
    } else {
        let c = crate::color::stdout_theme();
        let mut table = Table::new();
        table.load_preset(NOTHING);
        for t in &tasks {
            table.add_row(vec![
                format!("{}{}{}", c.bold, t.id, c.reset),
                t.title.clone(),
            ]);
        }
        if !tasks.is_empty() {
            println!("{table}");
        }
    }

    Ok(())
}
