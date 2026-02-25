use anyhow::{bail, Result};
use std::path::Path;

use crate::cli::DepAction;
use crate::db;

pub fn run(root: &Path, action: &DepAction, json: bool) -> Result<()> {
    let conn = db::open(root)?;

    match action {
        DepAction::Add { child, parent } => {
            if db::would_cycle(&conn, parent, child)? {
                bail!("adding dependency would create a cycle: {child} → {parent} → … → {child}");
            }
            conn.execute(
                "INSERT OR IGNORE INTO blockers (task_id, blocker_id) VALUES (?1, ?2)",
                [child, parent],
            )?;
            conn.execute(
                "UPDATE tasks SET updated = ?1 WHERE id = ?2",
                rusqlite::params![db::now_utc(), child],
            )?;
            if json {
                println!("{}", serde_json::json!({"child": child, "blocker": parent}));
            } else {
                let c = crate::color::stdout_theme();
                println!(
                    "{}{child}{} blocked by {}{parent}{}",
                    c.green, c.reset, c.yellow, c.reset
                );
            }
        }
        DepAction::Rm { child, parent } => {
            conn.execute(
                "DELETE FROM blockers WHERE task_id = ?1 AND blocker_id = ?2",
                [child, parent],
            )?;
            conn.execute(
                "UPDATE tasks SET updated = ?1 WHERE id = ?2",
                rusqlite::params![db::now_utc(), child],
            )?;
            if !json {
                let c = crate::color::stdout_theme();
                println!(
                    "{}{child}{} no longer blocked by {}{parent}{}",
                    c.green, c.reset, c.yellow, c.reset
                );
            }
        }
        DepAction::Tree { id } => {
            println!("{id}");
            let mut stmt = conn.prepare("SELECT id FROM tasks WHERE parent = ?1 ORDER BY id")?;
            let children: Vec<String> = stmt
                .query_map([id], |r| r.get(0))?
                .collect::<rusqlite::Result<_>>()?;
            for child in &children {
                println!("  {child}");
            }
        }
    }

    Ok(())
}
