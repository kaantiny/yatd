use anyhow::Result;
use std::path::Path;

use crate::db;

pub fn run(root: &Path) -> Result<()> {
    let store = db::open(root)?;
    let c = crate::color::stderr_theme();
    eprintln!(
        "{}info:{} compacting deltas into snapshot...",
        c.blue, c.reset
    );
    let removed = store.tidy()?;
    eprintln!("{}info:{} removed {removed} delta file(s)", c.blue, c.reset);
    Ok(())
}
