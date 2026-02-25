use anyhow::Result;
use serde::Deserialize;
use std::io::BufRead;
use std::path::Path;

use crate::db;

#[derive(Deserialize)]
struct ImportTask {
    id: String,
    title: String,
    #[serde(default)]
    description: String,
    #[serde(rename = "type", default = "default_type")]
    task_type: String,
    #[serde(default = "default_priority")]
    priority: i32,
    #[serde(default = "default_status")]
    status: String,
    #[serde(default)]
    parent: String,
    created: String,
    updated: String,
    #[serde(default)]
    labels: Vec<String>,
    #[serde(default)]
    blockers: Vec<String>,
}

fn default_type() -> String {
    "task".into()
}
fn default_priority() -> i32 {
    2
}
fn default_status() -> String {
    "open".into()
}

pub fn run(root: &Path, file: &str) -> Result<()> {
    let conn = db::open(root)?;

    eprintln!("info: importing from {file}...");

    let reader: Box<dyn BufRead> = if file == "-" {
        Box::new(std::io::stdin().lock())
    } else {
        Box::new(std::io::BufReader::new(std::fs::File::open(file)?))
    };

    for line in reader.lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }

        let t: ImportTask = serde_json::from_str(&line)?;

        conn.execute(
            "INSERT OR REPLACE INTO tasks
             (id, title, description, type, priority, status, parent, created, updated)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            rusqlite::params![
                t.id,
                t.title,
                t.description,
                t.task_type,
                t.priority,
                t.status,
                t.parent,
                t.created,
                t.updated,
            ],
        )?;

        // Replace labels.
        conn.execute("DELETE FROM labels WHERE task_id = ?1", [&t.id])?;
        for lbl in &t.labels {
            conn.execute(
                "INSERT INTO labels (task_id, label) VALUES (?1, ?2)",
                [&t.id, lbl],
            )?;
        }

        // Replace blockers.
        conn.execute("DELETE FROM blockers WHERE task_id = ?1", [&t.id])?;
        for blk in &t.blockers {
            conn.execute(
                "INSERT INTO blockers (task_id, blocker_id) VALUES (?1, ?2)",
                [&t.id, blk],
            )?;
        }
    }

    eprintln!("info: import complete");
    Ok(())
}
