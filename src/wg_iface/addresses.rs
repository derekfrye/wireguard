use crate::config::ResolvedConfig;
use crate::wg_iface::util::ignore_exists;
use anyhow::{Context, Result};
use ipnet::{IpNet, Ipv4Net, Ipv6Net};
use std::net::IpAddr;

pub(super) async fn configure_addresses(
    handle: &rtnetlink::Handle,
    link_index: u32,
    config: &ResolvedConfig,
) -> Result<()> {
    let (v4, v6) = server_addresses(config)?;
    let mut addrs = vec![IpNet::new(IpAddr::V4(v4), 32)?];
    if let Some(addr) = v6 {
        addrs.push(IpNet::new(IpAddr::V6(addr), 128)?);
    }

    for addr in addrs {
        let res = handle
            .address()
            .add(link_index, addr.addr(), addr.prefix_len())
            .execute()
            .await;
        ignore_exists(res).with_context(|| format!("adding address {addr}"))?;
    }
    Ok(())
}

fn server_addresses(
    config: &ResolvedConfig,
) -> Result<(std::net::Ipv4Addr, Option<std::net::Ipv6Addr>)> {
    let v4_net: Ipv4Net = config
        .network
        .subnet_v4
        .parse()
        .context("parsing subnet_v4")?;
    let mut v4_hosts = v4_net.hosts();
    let server_v4 = v4_hosts
        .next()
        .context("subnet_v4 has no usable host address")?;

    let server_v6 = match config.network.subnet_v6.as_deref() {
        Some(value) => {
            let v6_net: Ipv6Net = value.parse().context("parsing subnet_v6")?;
            let mut v6_hosts = v6_net.hosts();
            Some(
                v6_hosts
                    .next()
                    .context("subnet_v6 has no usable host address")?,
            )
        }
        None => None,
    };

    Ok((server_v4, server_v6))
}
