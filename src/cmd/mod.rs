mod create;
mod dep;
mod done;
mod export;
mod import;
mod init;
mod label;
mod list;
mod log;
mod next;
mod projects;
mod ready;
mod reopen;
mod rm;
mod search;
mod show;
mod skill;
mod stats;
pub mod sync;
mod tidy;
mod update;
mod r#use;

use crate::cli::{Cli, Command};
use crate::db;
use anyhow::Result;

fn require_root() -> Result<std::path::PathBuf> {
    std::env::current_dir().map_err(Into::into)
}

pub fn dispatch(cli: &Cli) -> Result<()> {
    if let Some(project) = &cli.project {
        std::env::set_var(db::PROJECT_ENV, project);
    }

    match &cli.command {
        Command::Init { name } => {
            let root = std::env::current_dir()?;
            init::run(&root, name, cli.json)
        }
        Command::Use { name } => r#use::run(name, cli.json),
        Command::Projects => projects::run(cli.json),
        Command::Create {
            title,
            priority,
            effort,
            task_type,
            desc,
            parent,
            labels,
        } => {
            let root = require_root()?;
            create::run(
                &root,
                create::Opts {
                    title: title.as_deref(),
                    priority: db::parse_priority(priority)?,
                    effort: db::parse_effort(effort)?,
                    task_type,
                    desc: desc.as_deref(),
                    parent: parent.as_deref(),
                    labels: labels.as_deref(),
                    json: cli.json,
                },
            )
        }
        Command::List {
            status,
            priority,
            effort,
            label,
        } => {
            let root = require_root()?;
            let pri = priority.as_deref().map(db::parse_priority).transpose()?;
            let eff = effort.as_deref().map(db::parse_effort).transpose()?;
            list::run(
                &root,
                status.as_deref(),
                pri,
                eff,
                label.as_deref(),
                cli.json,
            )
        }
        Command::Show { id } => {
            let root = require_root()?;
            show::run(&root, id, cli.json)
        }
        Command::Log { id, message } => {
            let root = require_root()?;
            log::run(&root, id, message, cli.json)
        }
        Command::Update {
            id,
            status,
            priority,
            effort,
            title,
            desc,
        } => {
            let root = require_root()?;
            let pri = priority.as_deref().map(db::parse_priority).transpose()?;
            let eff = effort.as_deref().map(db::parse_effort).transpose()?;
            update::run(
                &root,
                id,
                update::Opts {
                    status: status.as_deref(),
                    priority: pri,
                    effort: eff,
                    title: title.as_deref(),
                    desc: desc.as_deref(),
                    json: cli.json,
                },
            )
        }
        Command::Done { ids } => {
            let root = require_root()?;
            done::run(&root, ids, cli.json)
        }
        Command::Rm {
            force,
            recursive,
            ids,
        } => {
            let root = require_root()?;
            rm::run(&root, ids, *recursive, *force, cli.json)
        }
        Command::Reopen { ids } => {
            let root = require_root()?;
            reopen::run(&root, ids, cli.json)
        }
        Command::Dep { action } => {
            let root = require_root()?;
            dep::run(&root, action, cli.json)
        }
        Command::Label { action } => {
            let root = require_root()?;
            label::run(&root, action, cli.json)
        }
        Command::Search { query } => {
            let root = require_root()?;
            search::run(&root, query, cli.json)
        }
        Command::Ready => {
            let root = require_root()?;
            ready::run(&root, cli.json)
        }
        Command::Next {
            mode,
            verbose,
            limit,
        } => {
            let root = require_root()?;
            next::run(&root, mode, *verbose, *limit, cli.json)
        }
        Command::Stats => {
            let root = require_root()?;
            stats::run(&root)
        }
        Command::Tidy => {
            let root = require_root()?;
            tidy::run(&root)
        }
        Command::Export => {
            let root = require_root()?;
            export::run(&root)
        }
        Command::Import { file } => {
            let root = require_root()?;
            import::run(&root, file)
        }
        Command::Sync { code } => {
            let root = require_root()?;
            sync::run(&root, code.as_deref(), cli.json)
        }
        Command::Skill { dir } => skill::run(dir.as_deref()),
    }
}
