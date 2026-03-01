use anyhow::Result;

pub fn run(name: &str, json: bool) -> Result<()> {
    let cwd = std::env::current_dir()?;
    crate::db::use_project(&cwd, name)?;

    if json {
        println!(
            "{}",
            serde_json::json!({"success": true, "project": name, "bound_path": cwd})
        );
    } else {
        let c = crate::color::stdout_theme();
        println!("{}bound{} {} -> {name}", c.green, c.reset, cwd.display());
    }

    Ok(())
}
