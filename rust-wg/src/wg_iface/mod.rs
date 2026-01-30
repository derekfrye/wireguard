use anyhow::{Context, Result};

use crate::config::ResolvedConfig;

mod addresses;
mod link;
mod peers;
mod routes;
mod util;

pub const WG_IFACE: &str = "wg0";

pub struct WgHandles {
    pub link_index: u32,
}

pub async fn apply(config: &ResolvedConfig) -> Result<WgHandles> {
    let (connection, handle, _) = rtnetlink::new_connection().context("opening netlink")?;
    tokio::spawn(connection);

    eprintln!("wg: creating interface {}", WG_IFACE);
    let link_index = link::ensure_wireguard_link(&handle).await?;
    eprintln!("wg: interface {} index {}", WG_IFACE, link_index);
    eprintln!("wg: assigning interface addresses");
    addresses::configure_addresses(&handle, link_index, config).await?;
    eprintln!("wg: configuring peers");
    peers::configure_peers(config)?;
    eprintln!("wg: bringing interface up");
    link::set_link_up(&handle, link_index).await?;
    eprintln!("wg: adding peer routes");
    routes::configure_routes(&handle, link_index, config).await?;

    Ok(WgHandles { link_index })
}

pub async fn teardown(config: &ResolvedConfig, handle: WgHandles) -> Result<()> {
    let (connection, netlink, _) = rtnetlink::new_connection().context("opening netlink")?;
    tokio::spawn(connection);

    peers::best_effort_wg_cleanup(config)?;
    routes::delete_routes(&netlink, handle.link_index, config).await?;
    let res = netlink.link().del(handle.link_index).execute().await;
    util::ignore_notfound(res).context("deleting wg link")?;
    Ok(())
}
