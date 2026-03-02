use anyhow::Result;
use std::path::Path;

use crate::db;

pub fn run(root: &Path) -> Result<()> {
    let store = db::open(root)?;
    let c = crate::color::stderr_theme();
    eprintln!("{}info:{} writing compacted snapshot...", c.blue, c.reset);
    let out = store.write_snapshot()?;
    let removed = store.purge_deltas()?;
    eprintln!("{}info:{} wrote {}", c.blue, c.reset, out.display());
    eprintln!("{}info:{} removed {removed} delta file(s)", c.blue, c.reset);
    Ok(())
}
