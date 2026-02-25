use anyhow::Result;
use comfy_table::presets::NOTHING;
use comfy_table::Table;
use std::path::Path;

use crate::db;

pub fn run(
    root: &Path,
    status: Option<&str>,
    priority: Option<i32>,
    effort: Option<i32>,
    label: Option<&str>,
    json: bool,
) -> Result<()> {
    let conn = db::open(root)?;

    let mut sql = String::from(
        "SELECT id, title, description, type, priority, status, effort, parent, created, updated
         FROM tasks WHERE 1=1",
    );
    let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();
    let mut idx = 1;

    if let Some(s) = status {
        sql.push_str(&format!(" AND status = ?{idx}"));
        params.push(Box::new(s.to_string()));
        idx += 1;
    }
    if let Some(p) = priority {
        sql.push_str(&format!(" AND priority = ?{idx}"));
        params.push(Box::new(p));
        idx += 1;
    }
    if let Some(e) = effort {
        sql.push_str(&format!(" AND effort = ?{idx}"));
        params.push(Box::new(e));
        idx += 1;
    }
    if let Some(l) = label {
        sql.push_str(&format!(
            " AND id IN (SELECT task_id FROM labels WHERE label = ?{idx})"
        ));
        params.push(Box::new(l.to_string()));
    }

    sql.push_str(" ORDER BY priority, created");

    let param_refs: Vec<&dyn rusqlite::types::ToSql> = params.iter().map(|p| p.as_ref()).collect();
    let mut stmt = conn.prepare(&sql)?;
    let tasks: Vec<db::Task> = stmt
        .query_map(param_refs.as_slice(), db::row_to_task)?
        .collect::<rusqlite::Result<_>>()?;

    if json {
        let details: Vec<db::TaskDetail> = tasks
            .into_iter()
            .map(|t| {
                let labels = db::load_labels(&conn, &t.id)?;
                let blockers = db::load_blockers(&conn, &t.id)?;
                Ok(db::TaskDetail {
                    task: t,
                    labels,
                    blockers,
                })
            })
            .collect::<Result<_>>()?;
        println!("{}", serde_json::to_string(&details)?);
    } else {
        let c = crate::color::stdout_theme();
        let mut table = Table::new();
        table.load_preset(NOTHING);
        for t in &tasks {
            table.add_row(vec![
                format!("{}{}{}", c.bold, t.id, c.reset),
                format!("{}[{}]{}", c.yellow, t.status, c.reset),
                format!("{}{}{}", c.red, db::priority_label(t.priority), c.reset),
                format!("{}{}{}", c.blue, db::effort_label(t.effort), c.reset),
                t.title.clone(),
            ]);
        }
        if !tasks.is_empty() {
            println!("{table}");
        }
    }

    Ok(())
}
