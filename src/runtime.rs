use anyhow::Result;

use crate::{config, dns, module_check, nft, shutdown, wg_iface};

pub async fn run() -> Result<()> {
    module_check::ensure_wireguard_support().await?;
    let resolved = config::prepare()?;
    let wg_handle = wg_iface::apply(&resolved).await?;
    let nft_handles = nft::apply(&resolved)?;
    let coredns = dns::maybe_start(resolved.runtime.enable_coredns)?;

    shutdown::wait_for_signal().await?;

    if let Some(child) = coredns {
        dns::stop(child);
    }
    nft::teardown(&nft_handles)?;
    wg_iface::teardown(&resolved, wg_handle).await?;

    Ok(())
}

pub fn show_peer(_peers: Vec<String>) {
    // TODO: implement peer QR output and config lookup.
}

pub fn generate() -> Result<()> {
    let _ = config::prepare()?;
    Ok(())
}
