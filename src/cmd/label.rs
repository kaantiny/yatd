use anyhow::Result;
use std::path::Path;

use crate::cli::LabelAction;
use crate::db;

pub fn run(root: &Path, action: &LabelAction, json: bool) -> Result<()> {
    let conn = db::open(root)?;

    match action {
        LabelAction::Add { id, label } => {
            conn.execute(
                "INSERT OR IGNORE INTO labels (task_id, label) VALUES (?1, ?2)",
                [id, label],
            )?;
            conn.execute(
                "UPDATE tasks SET updated = ?1 WHERE id = ?2",
                rusqlite::params![db::now_utc(), id],
            )?;
            if json {
                println!("{}", serde_json::json!({"id": id, "label": label}));
            } else {
                let c = crate::color::stdout_theme();
                println!("{}added{} label {label}", c.green, c.reset);
            }
        }
        LabelAction::Rm { id, label } => {
            conn.execute(
                "DELETE FROM labels WHERE task_id = ?1 AND label = ?2",
                [id, label],
            )?;
            conn.execute(
                "UPDATE tasks SET updated = ?1 WHERE id = ?2",
                rusqlite::params![db::now_utc(), id],
            )?;
            if !json {
                let c = crate::color::stdout_theme();
                println!("{}removed{} label {label}", c.green, c.reset);
            }
        }
        LabelAction::List { id } => {
            let labels = db::load_labels(&conn, id)?;
            if json {
                println!("{}", serde_json::to_string(&labels)?);
            } else {
                for l in &labels {
                    println!("{l}");
                }
            }
        }
        LabelAction::ListAll => {
            let mut stmt = conn.prepare("SELECT DISTINCT label FROM labels ORDER BY label")?;
            let labels: Vec<String> = stmt
                .query_map([], |r| r.get(0))?
                .collect::<rusqlite::Result<_>>()?;
            if json {
                println!("{}", serde_json::to_string(&labels)?);
            } else {
                for l in &labels {
                    println!("{l}");
                }
            }
        }
    }

    Ok(())
}
