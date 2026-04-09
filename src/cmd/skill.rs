use anyhow::{bail, Result};
use std::path::PathBuf;

const SKILL_CONTENT: &str = include_str!("../../SKILL.md");
const SKILL_DIR: &str = "managing-tasks-with-td";
const SKILL_FILE: &str = "SKILL.md";

// Workflow skill contents - embedded in binary via include_str!
const TD_PLAN_SKILL: &str = include_str!("../../skills/td-plan/SKILL.md");
const TD_DECOMPOSE_SKILL: &str = include_str!("../../skills/td-decompose/SKILL.md");
const TD_REVIEW_SKILL: &str = include_str!("../../skills/td-review/SKILL.md");
const TD_SPEC_SKILL: &str = include_str!("../../skills/td-spec/SKILL.md");
const TD_EXECUTE_SKILL: &str = include_str!("../../skills/td-execute/SKILL.md");

const WORKFLOW_SKILLS: &[(&str, &str)] = &[
    ("td-plan", TD_PLAN_SKILL),
    ("td-decompose", TD_DECOMPOSE_SKILL),
    ("td-review", TD_REVIEW_SKILL),
    ("td-spec", TD_SPEC_SKILL),
    ("td-execute", TD_EXECUTE_SKILL),
];

fn default_skills_dir() -> Result<PathBuf> {
    if let Ok(xdg) = std::env::var("XDG_CONFIG_HOME") {
        Ok(PathBuf::from(xdg).join("agents/skills"))
    } else if let Ok(home) = std::env::var("HOME") {
        Ok(PathBuf::from(home).join(".config/agents/skills"))
    } else {
        bail!("neither $XDG_CONFIG_HOME nor $HOME is set");
    }
}

fn install_skill(skills_dir: &PathBuf, name: &str, content: &str) -> Result<()> {
    let dest_dir = skills_dir.join(name);
    std::fs::create_dir_all(&dest_dir)?;

    let dest = dest_dir.join(SKILL_FILE);
    std::fs::write(&dest, content)?;

    let c = crate::color::stderr_theme();
    eprintln!("{}info:{} wrote {}", c.blue, c.reset, dest.display());

    Ok(())
}

fn install_skills_from_dir(source_dir: &PathBuf, dest_dir: &PathBuf) -> Result<()> {
    for entry in std::fs::read_dir(source_dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            let skill_file = path.join("SKILL.md");
            if skill_file.exists() {
                let skill_name = path.file_name().unwrap().to_string_lossy();
                let content = std::fs::read_to_string(&skill_file)?;
                install_skill(dest_dir, &skill_name, &content)?;
            }
        }
    }
    Ok(())
}

pub fn run(
    dir: Option<&str>,
    list: bool,
    install: Option<&str>,
    base_only: bool,
    from_dir: Option<&str>,
) -> Result<()> {
    let skills_dir = match dir {
        Some(d) => PathBuf::from(d),
        None => default_skills_dir()?,
    };

    if list {
        println!("Available skills:");
        println!("  managing-tasks-with-td  (base skill)");
        for (name, _) in WORKFLOW_SKILLS {
            println!("  {}", name);
        }
        println!();
        println!("Usage:");
        println!("  td skill                       # Install all embedded skills");
        println!("  td skill --base-only           # Install only base skill");
        println!("  td skill --install td-plan     # Install specific skill");
        println!("  td skill --dir ./my-skills     # Install to custom directory");
        println!("  td skill --from-dir ./skills   # Install from local directory");
        return Ok(());
    }

    // Install from local directory
    if let Some(source) = from_dir {
        let source_path = PathBuf::from(source);
        if !source_path.exists() {
            bail!("source directory does not exist: {}", source);
        }
        install_skills_from_dir(&source_path, &skills_dir)?;
        return Ok(());
    }

    if base_only {
        // Install only base skill
        install_skill(&skills_dir, SKILL_DIR, SKILL_CONTENT)?;
        return Ok(());
    }

    if let Some(skill_name) = install {
        // Install specific skill
        if skill_name == "base" || skill_name == SKILL_DIR {
            install_skill(&skills_dir, SKILL_DIR, SKILL_CONTENT)?;
            return Ok(());
        }
        for (name, content) in WORKFLOW_SKILLS {
            if *name == skill_name {
                install_skill(&skills_dir, name, content)?;
                return Ok(());
            }
        }
        bail!("unknown skill: {}", skill_name);
    }

    // Default: install ALL skills (base + workflow)
    install_skill(&skills_dir, SKILL_DIR, SKILL_CONTENT)?;
    for (name, content) in WORKFLOW_SKILLS {
        install_skill(&skills_dir, name, content)?;
    }

    Ok(())
}
