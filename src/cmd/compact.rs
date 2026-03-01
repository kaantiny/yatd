use anyhow::Result;
use std::path::Path;

use crate::db;

pub fn run(root: &Path) -> Result<()> {
    let store = db::open(root)?;
    let c = crate::color::stderr_theme();
    eprintln!("{}info:{} writing compacted snapshot...", c.blue, c.reset);
    let out = store.write_snapshot()?;
    eprintln!("{}info:{} wrote {}", c.blue, c.reset, out.display());
    Ok(())
}
