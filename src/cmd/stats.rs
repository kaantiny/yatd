use anyhow::Result;
use std::path::Path;

use crate::db;

pub fn run(root: &Path) -> Result<()> {
    let conn = db::open(root)?;

    let total: i64 = conn.query_row("SELECT COUNT(*) FROM tasks", [], |r| r.get(0))?;
    let open: i64 = conn.query_row(
        "SELECT COUNT(*) FROM tasks WHERE status = 'open'",
        [],
        |r| r.get(0),
    )?;
    let in_progress: i64 = conn.query_row(
        "SELECT COUNT(*) FROM tasks WHERE status = 'in_progress'",
        [],
        |r| r.get(0),
    )?;
    let closed: i64 = conn.query_row(
        "SELECT COUNT(*) FROM tasks WHERE status = 'closed'",
        [],
        |r| r.get(0),
    )?;

    println!(
        "{}",
        serde_json::json!({
            "total": total,
            "open": open,
            "in_progress": in_progress,
            "closed": closed,
        })
    );

    Ok(())
}
