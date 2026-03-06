pub mod cli;
pub mod cmd;
pub mod color;
pub mod db;
pub mod editor;
pub mod migrate;
pub mod score;

use clap::Parser;

pub fn run() -> anyhow::Result<()> {
    let cli = cli::Cli::parse();
    cmd::dispatch(&cli)
}
