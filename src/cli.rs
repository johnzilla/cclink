use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "cclink", version, about = "Hand off a Claude Code session to another machine via Pubky")]
pub struct Cli {
    /// Claude Code session ID to publish (auto-discovers most recent if omitted)
    #[arg(value_name = "SESSION_ID")]
    pub session_id: Option<String>,

    /// Time-to-live in seconds (default: 86400 = 24 hours)
    #[arg(long, default_value = "86400")]
    pub ttl: u64,

    /// Render a QR code in the terminal after publish
    #[arg(long)]
    pub qr: bool,

    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Initialize or import a PKARR keypair
    Init(InitArgs),
    /// Show identity (public key, homeserver, fingerprint)
    Whoami,
    /// Pick up a Claude Code session handoff from the homeserver
    Pickup(PickupArgs),
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

#[derive(Parser)]
pub struct PickupArgs {
    /// z32-encoded public key of the Claude Code session publisher (defaults to own key)
    #[arg(value_name = "PUBKEY")]
    pub pubkey: Option<String>,

    /// Skip confirmation prompt and launch immediately
    #[arg(long, short = 'y')]
    pub yes: bool,

    /// Render a QR code showing the session ID
    #[arg(long)]
    pub qr: bool,
}
