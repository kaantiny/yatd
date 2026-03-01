use anyhow::Result;
use comfy_table::presets::NOTHING;
use comfy_table::{Cell, Table};
use std::path::Path;

use crate::color::{cell_bold, stdout_use_color};
use crate::db;

pub fn run(root: &Path, query: &str, json: bool) -> Result<()> {
    let store = db::open(root)?;
    let q = query.to_lowercase();

    let tasks: Vec<db::Task> = store
        .list_tasks()?
        .into_iter()
        .filter(|t| {
            t.title.to_lowercase().contains(&q) || t.description.to_lowercase().contains(&q)
        })
        .collect();

    if json {
        println!("{}", serde_json::to_string(&tasks)?);
    } else {
        let use_color = stdout_use_color();
        let mut table = Table::new();
        table.load_preset(NOTHING);
        for t in &tasks {
            table.add_row(vec![cell_bold(&t.id, use_color), Cell::new(&t.title)]);
        }
        if !tasks.is_empty() {
            println!("{table}");
        }
    }

    Ok(())
}
