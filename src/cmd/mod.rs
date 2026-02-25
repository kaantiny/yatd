mod compact;
mod create;
mod dep;
mod done;
mod export;
mod import;
mod init;
mod label;
mod list;
mod ready;
mod reopen;
mod search;
mod show;
mod skill;
mod stats;
mod update;

use crate::cli::{Cli, Command};
use crate::db;
use anyhow::Result;

fn require_root() -> Result<std::path::PathBuf> {
    db::find_root(&std::env::current_dir()?)
}

pub fn dispatch(cli: &Cli) -> Result<()> {
    match &cli.command {
        Command::Init { stealth } => {
            let root = std::env::current_dir()?;
            init::run(&root, *stealth, cli.json)
        }
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
                    priority: *priority,
                    effort: *effort,
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
            label,
        } => {
            let root = require_root()?;
            list::run(
                &root,
                status.as_deref(),
                *priority,
                label.as_deref(),
                cli.json,
            )
        }
        Command::Show { id } => {
            let root = require_root()?;
            show::run(&root, id, cli.json)
        }
        Command::Update {
            id,
            status,
            priority,
            title,
            desc,
        } => {
            let root = require_root()?;
            update::run(
                &root,
                id,
                update::Opts {
                    status: status.as_deref(),
                    priority: *priority,
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
        Command::Stats => {
            let root = require_root()?;
            stats::run(&root)
        }
        Command::Compact => {
            let root = require_root()?;
            compact::run(&root)
        }
        Command::Export => {
            let root = require_root()?;
            export::run(&root)
        }
        Command::Import { file } => {
            let root = require_root()?;
            import::run(&root, file)
        }
        Command::Skill { dir } => skill::run(dir.as_deref()),
    }
}
