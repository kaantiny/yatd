use anyhow::{bail, Result};
use comfy_table::presets::NOTHING;
use comfy_table::Table;
use std::path::Path;

use crate::db;
use crate::score::{self, Mode};

/// Parse the mode string from the CLI.
fn parse_mode(s: &str) -> Result<Mode> {
    match s {
        "impact" => Ok(Mode::Impact),
        "effort" => Ok(Mode::Effort),
        _ => bail!("invalid mode '{s}': expected impact or effort"),
    }
}

pub fn run(root: &Path, mode_str: &str, verbose: bool, limit: usize, json: bool) -> Result<()> {
    let mode = parse_mode(mode_str)?;
    let conn = db::open(root)?;

    // Load all open tasks.
    let mut stmt = conn.prepare(
        "SELECT id, title, priority, effort
         FROM tasks
         WHERE status = 'open'",
    )?;
    let open_tasks: Vec<(String, String, i32, i32)> = stmt
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))?
        .collect::<rusqlite::Result<_>>()?;

    // Load all blocker edges between open tasks.
    let mut edge_stmt = conn.prepare(
        "SELECT b.task_id, b.blocker_id
         FROM blockers b
         JOIN tasks t1 ON b.task_id = t1.id
         JOIN tasks t2 ON b.blocker_id = t2.id
         WHERE t1.status = 'open' AND t2.status = 'open'",
    )?;
    let edges: Vec<(String, String)> = edge_stmt
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
        .collect::<rusqlite::Result<_>>()?;

    let scored = score::rank(&open_tasks, &edges, mode, limit);

    if scored.is_empty() {
        if json {
            println!("[]");
        } else {
            println!("No open tasks.");
        }
        return Ok(());
    }

    if json {
        let items: Vec<serde_json::Value> = scored
            .iter()
            .enumerate()
            .map(|(i, s)| {
                serde_json::json!({
                    "rank": i + 1,
                    "id": s.id,
                    "title": s.title,
                    "score": s.score,
                    "priority": db::priority_label(s.priority),
                    "effort": db::effort_label(s.effort),
                    "downstream_score": s.downstream_score,
                    "priority_weight": s.priority_weight,
                    "effort_weight": s.effort_weight,
                    "total_unblocked": s.total_unblocked,
                    "direct_unblocked": s.direct_unblocked,
                })
            })
            .collect();
        println!("{}", serde_json::to_string(&items)?);
    } else {
        let mut table = Table::new();
        table.load_preset(NOTHING);
        table.set_header(vec!["#", "ID", "SCORE", "TITLE"]);

        for (i, s) in scored.iter().enumerate() {
            let c = crate::color::stdout_theme();
            table.add_row(vec![
                format!("{}", i + 1),
                format!("{}{}{}", c.bold, s.id, c.reset),
                format!("{:.2}", s.score),
                s.title.clone(),
            ]);
        }
        println!("{table}");

        if verbose {
            let mode_label = match mode {
                Mode::Impact => "impact",
                Mode::Effort => "effort",
            };
            println!();
            println!("mode: {mode_label}");
            println!();
            for (i, s) in scored.iter().enumerate() {
                println!("{}. {} — score: {:.2}", i + 1, s.id, s.score);
                match mode {
                    Mode::Impact => {
                        println!(
                            "   ({:.2} + 1.00) × {:.0} / {:.0} = {:.2}",
                            s.downstream_score, s.priority_weight, s.effort_weight, s.score
                        );
                    }
                    Mode::Effort => {
                        println!(
                            "   ({:.2} × 0.25 + 1.00) × {:.0} / {:.0}² = {:.2}",
                            s.downstream_score, s.priority_weight, s.effort_weight, s.score
                        );
                    }
                }
                let unblocked = if s.direct_unblocked == s.total_unblocked {
                    format!("{} tasks", s.total_unblocked)
                } else {
                    format!(
                        "{} tasks ({} directly)",
                        s.total_unblocked, s.direct_unblocked
                    )
                };
                println!("   Unblocks: {unblocked}");
            }
        }
    }

    Ok(())
}
