use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "rust-wg", version, about = "WireGuard container runtime")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    Run,
    ShowPeer {
        #[arg(required = true)]
        peers: Vec<String>,
    },
    Generate,
}
