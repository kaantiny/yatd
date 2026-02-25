use anyhow::Result;
use std::path::Path;

use crate::db;

pub fn run(root: &Path, ids: &[String], json: bool) -> Result<()> {
    let conn = db::open(root)?;
    let ts = db::now_utc();

    let c = crate::color::stdout_theme();
    for id in ids {
        conn.execute(
            "UPDATE tasks SET status = 'open', updated = ?1 WHERE id = ?2",
            rusqlite::params![ts, id],
        )?;
        if !json {
            println!("{}reopened{} {id}", c.green, c.reset);
        }
    }

    if json {
        let details: Vec<serde_json::Value> = ids
            .iter()
            .map(|id| {
                Ok(serde_json::json!({
                    "id": id,
                    "status": "open",
                }))
            })
            .collect::<Result<_>>()?;
        println!("{}", serde_json::to_string(&details)?);
    }

    Ok(())
}
