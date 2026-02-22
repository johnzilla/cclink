use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "cclink", version, about = "Secure session handoff via Pubky")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Initialize or import a PKARR keypair
    Init(InitArgs),
    /// Show identity (public key, homeserver, fingerprint)
    Whoami,
}

#[derive(Parser)]
pub struct InitArgs {
    /// Import an existing keypair from file path or stdin (use - for stdin)
    #[arg(long, value_name = "PATH")]
    pub import: Option<String>,

    /// Homeserver URL to associate with this keypair
    #[arg(long, default_value = "https://pubky.app")]
    pub homeserver: String,

    /// Skip overwrite confirmation prompt
    #[arg(long, short = 'y')]
    pub yes: bool,
}
