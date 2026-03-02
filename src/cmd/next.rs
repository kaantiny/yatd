use anyhow::{bail, Result};
use comfy_table::presets::NOTHING;
use comfy_table::{Cell, Table};
use std::collections::HashSet;
use std::path::Path;

use crate::color::{cell_bold, stdout_use_color};
use crate::db;
use crate::score::{self, Mode};

fn parse_mode(s: &str) -> Result<Mode> {
    match s {
        "impact" => Ok(Mode::Impact),
        "effort" => Ok(Mode::Effort),
        _ => bail!("invalid mode '{s}': expected impact or effort"),
    }
}

pub fn run(root: &Path, mode_str: &str, verbose: bool, limit: usize, json: bool) -> Result<()> {
    let mode = parse_mode(mode_str)?;
    let store = db::open(root)?;
    let all = store.list_tasks()?;

    let open_tasks: Vec<score::TaskInput> = all
        .iter()
        .filter(|t| t.status == db::Status::Open)
        .map(|t| score::TaskInput {
            id: t.id.as_str().to_string(),
            title: t.title.clone(),
            priority_score: t.priority.score(),
            effort_score: t.effort.score(),
            priority_label: db::priority_label(t.priority).to_string(),
            effort_label: db::effort_label(t.effort).to_string(),
        })
        .collect();

    let edges: Vec<(String, String)> = all
        .iter()
        .filter(|t| t.status == db::Status::Open)
        .flat_map(|t| {
            t.blockers
                .iter()
                .map(|b| (t.id.as_str().to_string(), b.as_str().to_string()))
                .collect::<Vec<_>>()
        })
        .collect();

    let parents_with_open_children: HashSet<String> = all
        .iter()
        .filter(|t| t.status == db::Status::Open)
        .filter_map(|t| t.parent.as_ref().map(|p| p.as_str().to_string()))
        .collect();

    let scored = score::rank(
        &open_tasks,
        &edges,
        &parents_with_open_children,
        mode,
        limit,
    );

    if scored.is_empty() {
        if json {
            println!("[]");
        } else {
            println!("No open tasks.");
        }
        return Ok(());
    }

    if json {
        let out: Vec<_> = scored
            .iter()
            .enumerate()
            .map(|(i, s)| {
                serde_json::json!({
                    "rank": i + 1,
                    "id": s.id,
                    "title": s.title,
                    "score": s.score,
                    "priority": s.priority,
                    "effort": s.effort,
                    "downstream_score": s.downstream_score,
                    "priority_weight": s.priority_weight,
                    "effort_weight": s.effort_weight,
                    "total_unblocked": s.total_unblocked,
                    "direct_unblocked": s.direct_unblocked,
                })
            })
            .collect();
        println!("{}", serde_json::to_string(&out)?);
    } else {
        let use_color = stdout_use_color();
        let mut table = Table::new();
        table.load_preset(NOTHING);
        table.set_header(vec!["#", "ID", "SCORE", "TITLE"]);

        for (i, s) in scored.iter().enumerate() {
            let short = db::TaskId::display_id(&s.id);
            table.add_row(vec![
                Cell::new(i + 1),
                cell_bold(&short, use_color),
                Cell::new(format!("{:.2}", s.score)),
                Cell::new(&s.title),
            ]);
        }
        println!("{table}");

        if verbose {
            let mode_label = match mode {
                Mode::Impact => "impact",
                Mode::Effort => "effort",
            };
            println!("\nmode: {mode_label}");
            for (i, s) in scored.iter().enumerate() {
                let short = db::TaskId::display_id(&s.id);
                let formula = match mode {
                    Mode::Impact => format!(
                        "({:.2} + 1.00) × {:.2} / {:.2}^0.25 = {:.2}",
                        s.downstream_score, s.priority_weight, s.effort_weight, s.score
                    ),
                    Mode::Effort => format!(
                        "({:.2} × 0.25 + 1.00) × {:.2} / {:.2}² = {:.2}",
                        s.downstream_score, s.priority_weight, s.effort_weight, s.score
                    ),
                };
                let task_word = if s.total_unblocked == 1 {
                    "task"
                } else {
                    "tasks"
                };
                println!(
                    "\n{}. {}\n   {}\n   Unblocks: {} {} ({} directly)",
                    i + 1,
                    short,
                    formula,
                    s.total_unblocked,
                    task_word,
                    s.direct_unblocked
                );
            }
        }
    }

    Ok(())
}
