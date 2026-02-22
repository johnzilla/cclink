mod cli;
mod commands;
mod crypto;
mod error;
mod keys;
mod record;

use clap::Parser;
use cli::{Cli, Commands};

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Init(args) => commands::init::run_init(args)?,
        Commands::Whoami => commands::whoami::run_whoami()?,
    }

    Ok(())
}
