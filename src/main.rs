mod cli;
mod commands;
mod crypto;
mod error;
mod keys;
mod record;
mod session;
mod transport;
mod util;

use clap::Parser;
use cli::{Cli, Commands};

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Some(Commands::Init(args)) => commands::init::run_init(args)?,
        Some(Commands::Whoami) => commands::whoami::run_whoami()?,
        Some(Commands::Pickup(args)) => commands::pickup::run_pickup(args)?,
        Some(Commands::List) => commands::list::run_list()?,
        Some(Commands::Revoke(args)) => commands::revoke::run_revoke(args)?,
        None => commands::publish::run_publish(&cli)?,
    }

    Ok(())
}
