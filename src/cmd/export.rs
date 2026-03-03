use anyhow::Result;
use std::path::Path;

use crate::db;

pub fn run(root: &Path) -> Result<()> {
    let store = db::open(root)?;
    for task in store.list_tasks_unfiltered()? {
        println!("{}", task.to_export_value());
    }
    Ok(())
}
