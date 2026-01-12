mod cli;
mod db;
mod ingestion;
mod normalize;
mod query;

use anyhow::Result;
use clap::Parser;

use crate::cli::{Cli, Command, VariantsCommand};

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Command::Ingest(args) => ingestion::ingest::run(args),
        Command::Search(args) => query::run_search(args),
        Command::Show(args) => query::run_show(args),
        Command::Top(args) => query::run_top(args),
        Command::Variants { command } => match command {
            VariantsCommand::Build(args) => ingestion::variants::run_build(args),
            VariantsCommand::BuildEnsemble(args) => ingestion::variants::run_build_ensemble(args),
        },
    }
}
