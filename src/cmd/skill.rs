use anyhow::{bail, Result};
use std::path::PathBuf;

const SKILL_CONTENT: &str = include_str!("../../SKILL.md");
const SKILL_DIR: &str = "managing-tasks-with-td";
const SKILL_FILE: &str = "SKILL.md";

fn default_skills_dir() -> Result<PathBuf> {
    if let Ok(xdg) = std::env::var("XDG_CONFIG_HOME") {
        Ok(PathBuf::from(xdg).join("agents/skills"))
    } else if let Ok(home) = std::env::var("HOME") {
        Ok(PathBuf::from(home).join(".config/agents/skills"))
    } else {
        bail!("neither $XDG_CONFIG_HOME nor $HOME is set");
    }
}

pub fn run(dir: Option<&str>) -> Result<()> {
    let skills_dir = match dir {
        Some(d) => PathBuf::from(d),
        None => default_skills_dir()?,
    };

    let dest_dir = skills_dir.join(SKILL_DIR);
    std::fs::create_dir_all(&dest_dir)?;

    let dest = dest_dir.join(SKILL_FILE);
    std::fs::write(&dest, SKILL_CONTENT)?;

    let c = crate::color::stderr_theme();
    eprintln!("{}info:{} wrote {}", c.blue, c.reset, dest.display());

    Ok(())
}
