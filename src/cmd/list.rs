use anyhow::Result;
use comfy_table::presets::NOTHING;
use comfy_table::{Cell, Color, Table};
use std::path::Path;

use crate::color::{cell_bold, cell_fg, stdout_use_color};
use crate::db;

pub fn run(
    root: &Path,
    status: Option<&str>,
    priority: Option<db::Priority>,
    effort: Option<db::Effort>,
    label: Option<&str>,
    json: bool,
) -> Result<()> {
    let store = db::open(root)?;
    let mut tasks = store.list_tasks()?;

    if let Some(s) = status {
        let parsed = db::parse_status(s)?;
        tasks.retain(|t| t.status == parsed);
    }
    if let Some(p) = priority {
        tasks.retain(|t| t.priority == p);
    }
    if let Some(e) = effort {
        tasks.retain(|t| t.effort == e);
    }
    if let Some(l) = label {
        tasks.retain(|t| t.labels.iter().any(|x| x == l));
    }

    tasks.sort_by_key(|t| (t.priority.score(), t.created_at.clone()));

    if json {
        // Keep list JSON lean: include scheduling fields but not full work-log history.
        let mut value = serde_json::to_value(&tasks)?;
        if let Some(items) = value.as_array_mut() {
            for item in items {
                if let Some(obj) = item.as_object_mut() {
                    obj.remove("logs");
                }
            }
        }
        println!("{}", serde_json::to_string(&value)?);
    } else {
        let use_color = stdout_use_color();
        let mut table = Table::new();
        table.load_preset(NOTHING);
        table.set_header(vec!["ID", "STATUS", "PRIORITY", "EFFORT", "TITLE"]);
        for t in &tasks {
            table.add_row(vec![
                cell_bold(&t.id, use_color),
                cell_fg(
                    format!("[{}]", db::status_label(t.status)),
                    Color::Yellow,
                    use_color,
                ),
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
