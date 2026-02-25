use anyhow::Result;
use std::path::Path;

use crate::db;

pub fn run(root: &Path) -> Result<()> {
    let conn = db::open(root)?;
    let c = crate::color::stderr_theme();
    eprintln!("{}info:{} vacuuming database...", c.blue, c.reset);
    conn.execute_batch("VACUUM;")?;
    eprintln!("{}info:{} done", c.blue, c.reset);
    Ok(())
}
