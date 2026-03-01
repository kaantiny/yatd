use anyhow::Result;
use comfy_table::presets::NOTHING;
use comfy_table::{Cell, Color, Table};
use std::path::Path;

use crate::color::{cell_bold, cell_fg, stdout_use_color};
use crate::db;

pub fn run(root: &Path, json: bool) -> Result<()> {
    let store = db::open(root)?;

    let mut tasks = Vec::new();
    for task in store.list_tasks()? {
        if task.status != db::Status::Open {
            continue;
        }
        let blockers = db::partition_blockers(&store, &task.blockers)?;
        if blockers.open.is_empty() {
            tasks.push(task);
        }
    }

    tasks.sort_by_key(|t| (t.priority.score(), t.created_at.clone()));

    if json {
        println!("{}", serde_json::to_string(&tasks)?);
    } else {
        let use_color = stdout_use_color();
        let mut table = Table::new();
        table.load_preset(NOTHING);
        table.set_header(vec!["ID", "PRIORITY", "EFFORT", "TITLE"]);
        for t in &tasks {
            table.add_row(vec![
                cell_bold(&t.id, use_color),
                cell_fg(db::priority_label(t.priority), Color::Red, use_color),
                cell_fg(db::effort_label(t.effort), Color::Blue, use_color),
                Cell::new(&t.title),
            ]);
        }
        if !tasks.is_empty() {
            println!("{table}");
        }
    }

    Ok(())
}
