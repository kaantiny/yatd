use anyhow::Result;
use std::path::Path;

use crate::db;

pub fn run(root: &Path) -> Result<()> {
    let conn = db::open(root)?;

    let mut stmt = conn.prepare(
        "SELECT id, title, description, type, priority, status, effort, parent, created, updated
         FROM tasks ORDER BY id",
    )?;

    let tasks: Vec<db::Task> = stmt
        .query_map([], db::row_to_task)?
        .collect::<rusqlite::Result<_>>()?;

    for t in &tasks {
        let labels = db::load_labels(&conn, &t.id)?;
        let blockers = db::load_blockers(&conn, &t.id)?;
        let detail = db::TaskDetail {
            task: db::Task {
                id: t.id.clone(),
                title: t.title.clone(),
                description: t.description.clone(),
                task_type: t.task_type.clone(),
                priority: t.priority,
                status: t.status.clone(),
                effort: t.effort,
                parent: t.parent.clone(),
                created: t.created.clone(),
                updated: t.updated.clone(),
            },
            labels,
            blockers,
        };
        println!("{}", serde_json::to_string(&detail)?);
    }

    Ok(())
}
