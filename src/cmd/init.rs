use anyhow::{bail, Result};
use std::path::Path;

pub fn run(root: &Path, stealth: bool, json: bool) -> Result<()> {
    let td_dir = crate::db::td_dir(root);
    if td_dir.exists() {
        bail!("already initialized");
    }

    crate::db::init(root)?;

    if stealth {
        use std::io::Write;
        let mut f = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(root.join(".gitignore"))?;
        writeln!(f, ".td/")?;
    }

    let c = crate::color::stderr_theme();
    eprintln!("{}info:{} initialized .td/", c.blue, c.reset);
    if json {
        println!(r#"{{"success":true}}"#);
    }

    Ok(())
}
