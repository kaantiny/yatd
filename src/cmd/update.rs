use anyhow::Result;
use std::path::Path;

use crate::db;

pub struct Opts<'a> {
    pub status: Option<&'a str>,
    pub priority: Option<i32>,
    pub title: Option<&'a str>,
    pub desc: Option<&'a str>,
    pub json: bool,
}

pub fn run(root: &Path, id: &str, opts: Opts) -> Result<()> {
    let conn = db::open(root)?;
    let ts = db::now_utc();

    let mut sets = vec![format!("updated = '{ts}'")];
    let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();
    let mut idx = 1;

    if let Some(s) = opts.status {
        sets.push(format!("status = ?{idx}"));
        params.push(Box::new(s.to_string()));
        idx += 1;
    }
    if let Some(p) = opts.priority {
        sets.push(format!("priority = ?{idx}"));
        params.push(Box::new(p));
        idx += 1;
    }
    if let Some(t) = opts.title {
        sets.push(format!("title = ?{idx}"));
        params.push(Box::new(t.to_string()));
        idx += 1;
    }
    if let Some(d) = opts.desc {
        sets.push(format!("description = ?{idx}"));
        params.push(Box::new(d.to_string()));
        idx += 1;
    }

    let sql = format!("UPDATE tasks SET {} WHERE id = ?{idx}", sets.join(", "));
    params.push(Box::new(id.to_string()));

    let param_refs: Vec<&dyn rusqlite::types::ToSql> = params.iter().map(|p| p.as_ref()).collect();
    conn.execute(&sql, param_refs.as_slice())?;

    if opts.json {
        let detail = db::load_task_detail(&conn, id)?;
        println!("{}", serde_json::to_string(&detail)?);
    } else {
        let c = crate::color::stdout_theme();
        println!("{}updated{} {id}", c.green, c.reset);
    }

    Ok(())
}
