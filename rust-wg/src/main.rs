mod cli;
mod config;
mod dns;
mod netlink_util;
mod module_check;
mod nft;
mod runtime;
mod shutdown;
mod wg_iface;

use clap::Parser;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = cli::Cli::parse();

    match cli.command {
        cli::Commands::Run => runtime::run().await,
        cli::Commands::ShowPeer { peers } => runtime::show_peer(peers).await,
        cli::Commands::Generate => runtime::generate().await,
    }
}
