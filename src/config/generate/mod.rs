use crate::config::types::{ConfigFile, Paths, Peer};
use anyhow::{Context, Result};
use ipnet::{Ipv4Net, Ipv6Net};
use std::fs;

mod ip_alloc;
mod keys;
mod peer_conf;
mod server_conf;

pub(super) fn generate_all(cfg: &ConfigFile, peers: &[Peer], paths: &Paths) -> Result<()> {
    let server_keys = keys::ensure_server_keys(paths)?;

    let v4_net: Ipv4Net = cfg.network.subnet_v4.parse().context("parsing subnet_v4")?;
    let v6_net: Option<Ipv6Net> = match cfg.network.subnet_v6.as_deref() {
        Some(value) => Some(value.parse().context("parsing subnet_v6")?),
        None => None,
    };

    let mut assigned_v4 = ip_alloc::gather_assigned_ips(&paths.peers)?;
    let mut assigned_v6 = ip_alloc::gather_assigned_ips_v6(&paths.peers)?;

    let mut v4_hosts = v4_net.hosts();
    let server_v4 = v4_hosts
        .next()
        .context("subnet_v4 has no usable host address")?;
    assigned_v4.insert(server_v4.to_string());

    let server_v6 = if let Some(net) = v6_net.as_ref() {
        let mut hosts = net.hosts();
        let addr = hosts
            .next()
            .context("subnet_v6 has no usable host address")?;
        assigned_v6.insert(addr.to_string());
        Some(addr)
    } else {
        None
    };

    let mut peer_ips = Vec::new();
    for _ in peers {
        let ip = ip_alloc::next_available_v4(&mut v4_hosts, &assigned_v4)?;
        assigned_v4.insert(ip.to_string());
        let ip6 = if let Some(net) = v6_net.as_ref() {
            let mut hosts = net.hosts();
            let ip6 = ip_alloc::next_available_v6(&mut hosts, &assigned_v6)?;
            assigned_v6.insert(ip6.to_string());
            Some(ip6)
        } else {
            None
        };
        peer_ips.push((ip, ip6));
    }

    for peer in peers {
        let peer_dir = paths.peers.join(&peer.id);
        fs::create_dir_all(&peer_dir).context("creating peer dir")?;
        let _ = keys::ensure_peer_keys(&peer_dir)?;
    }

    server_conf::write_server_conf(
        cfg,
        paths,
        &server_keys.private,
        server_v4,
        server_v6,
        peers,
        &peer_ips,
    )?;

    for (peer, (ip, ip6)) in peers.iter().zip(peer_ips.into_iter()) {
        peer_conf::generate_peer(cfg, paths, peer, ip, ip6, &server_keys.public)?;
    }

    Ok(())
}
