use anyhow::Result;
use std::path::Path;

use crate::cli::ProjectAction;

pub fn run(action: &ProjectAction, json: bool) -> Result<()> {
    let cwd = std::env::current_dir()?;

    match action {
        ProjectAction::Init { name } => init(&cwd, name, json),
        ProjectAction::Bind { name } => bind(&cwd, name, json),
        ProjectAction::Unbind => unbind(&cwd, json),
        ProjectAction::Delete { name } => delete(name, json),
        ProjectAction::List => list(json),
    }
}

fn init(cwd: &Path, name: &str, json: bool) -> Result<()> {
    crate::db::init(cwd, name)?;

    if json {
        println!(
            "{}",
            serde_json::json!({"success": true, "project": name, "bound_path": cwd})
        );
    } else {
        let c = crate::color::stderr_theme();
        eprintln!("{}info:{} initialized project '{name}'", c.blue, c.reset);
    }

    Ok(())
}

fn bind(cwd: &Path, name: &str, json: bool) -> Result<()> {
    crate::db::use_project(cwd, name)?;

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

fn unbind(cwd: &Path, json: bool) -> Result<()> {
    crate::db::unbind_project(cwd)?;

    if json {
        println!("{}", serde_json::json!({"success": true}));
    } else {
        let c = crate::color::stderr_theme();
        eprintln!("{}info:{} unbound {}", c.blue, c.reset, cwd.display());
    }

    Ok(())
}

fn delete(name: &str, json: bool) -> Result<()> {
    crate::db::delete_project(name)?;

    if json {
        println!("{}", serde_json::json!({"success": true, "project": name}));
    } else {
        let c = crate::color::stderr_theme();
        eprintln!("{}info:{} deleted project '{name}'", c.blue, c.reset);
    }

    Ok(())
}

fn list(json: bool) -> Result<()> {
    let projects = crate::db::list_projects()?;

    if json {
        println!("{}", serde_json::to_string(&projects)?);
    } else {
        for project in projects {
            println!("{project}");
        }
    }

    Ok(())
}
