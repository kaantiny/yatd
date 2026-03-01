use anyhow::Result;
use std::path::Path;

pub fn run(root: &Path, name: &str, json: bool) -> Result<()> {
    crate::db::init(root, name)?;

    if json {
        println!(
            "{}",
            serde_json::json!({"success": true, "project": name, "bound_path": root})
        );
    } else {
        let c = crate::color::stderr_theme();
        eprintln!("{}info:{} initialized project '{name}'", c.blue, c.reset);
    }

    Ok(())
}
