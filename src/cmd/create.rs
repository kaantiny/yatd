use anyhow::Result;
use std::path::Path;

use crate::db;

pub struct Opts<'a> {
    pub title: Option<&'a str>,
    pub priority: i32,
    pub task_type: &'a str,
    pub desc: Option<&'a str>,
    pub parent: Option<&'a str>,
    pub labels: Option<&'a str>,
    pub json: bool,
}

pub fn run(root: &Path, opts: Opts) -> Result<()> {
    let title = opts
        .title
        .ok_or_else(|| anyhow::anyhow!("title required"))?;
    let desc = opts.desc.unwrap_or("");
    let ts = db::now_utc();

    let conn = db::open(root)?;

    let id = match opts.parent {
        Some(pid) => {
            let count: i64 =
                conn.query_row("SELECT COUNT(*) FROM tasks WHERE parent = ?1", [pid], |r| {
                    r.get(0)
                })?;
            format!("{pid}.{}", count + 1)
        }
        None => db::gen_id(),
    };

    conn.execute(
        "INSERT INTO tasks (id, title, description, type, priority, status, parent, created, updated)
         VALUES (?1, ?2, ?3, ?4, ?5, 'open', ?6, ?7, ?8)",
        rusqlite::params![
            id,
            title,
            desc,
            opts.task_type,
            opts.priority,
            opts.parent.unwrap_or(""),
            ts,
            ts
        ],
    )?;

    if let Some(label_str) = opts.labels {
        for lbl in label_str.split(',') {
            let lbl = lbl.trim();
            if !lbl.is_empty() {
                conn.execute(
                    "INSERT OR IGNORE INTO labels (task_id, label) VALUES (?1, ?2)",
                    [&id, lbl],
                )?;
            }
        }
    }

    if opts.json {
        let task = db::Task {
            id: id.clone(),
            title: title.to_string(),
            description: desc.to_string(),
            task_type: opts.task_type.to_string(),
            priority: opts.priority,
            status: "open".to_string(),
            parent: opts.parent.unwrap_or("").to_string(),
            created: ts.clone(),
            updated: ts,
        };
        println!("{}", serde_json::to_string(&task)?);
    } else {
        let c = crate::color::stdout_theme();
        println!("{}created{} {id}: {title}", c.green, c.reset);
    }

    Ok(())
}
