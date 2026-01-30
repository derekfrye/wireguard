use anyhow::{Context, Result};
use ipnet::{IpNet, Ipv4Net, Ipv6Net};
use rtnetlink::{LinkUnspec, LinkWireguard, RouteMessageBuilder};
use std::net::IpAddr;
use std::path::Path;
use std::process::Command;

use crate::config::ResolvedConfig;
use crate::netlink_util::get_link_by_name;

pub const WG_IFACE: &str = "wg0";

pub struct WgHandles {
    pub link_index: u32,
}

pub async fn apply(config: &ResolvedConfig) -> Result<WgHandles> {
    let (connection, handle, _) = rtnetlink::new_connection().context("opening netlink")?;
    tokio::spawn(connection);

    eprintln!("wg: creating interface {}", WG_IFACE);
    let link_index = ensure_wireguard_link(&handle).await?;
    eprintln!("wg: interface {} index {}", WG_IFACE, link_index);
    eprintln!("wg: assigning interface addresses");
    configure_addresses(&handle, link_index, config).await?;
    eprintln!("wg: configuring peers");
    configure_peers(config)?;
    eprintln!("wg: bringing interface up");
    set_link_up(&handle, link_index).await?;
    eprintln!("wg: adding peer routes");
    configure_routes(&handle, link_index, config).await?;

    Ok(WgHandles {
        link_index,
    })
}

pub async fn teardown(config: &ResolvedConfig, handle: WgHandles) -> Result<()> {
    let (connection, netlink, _) = rtnetlink::new_connection().context("opening netlink")?;
    tokio::spawn(connection);

    best_effort_wg_cleanup(config)?;
    delete_routes(&netlink, handle.link_index, config).await?;
    let res = netlink.link().del(handle.link_index).execute().await;
    ignore_notfound(res).context("deleting wg link")?;
    Ok(())
}

async fn ensure_wireguard_link(handle: &rtnetlink::Handle) -> Result<u32> {
    if let Some(link) = get_link_by_name(handle, WG_IFACE).await? {
        return Ok(link.header.index);
    }

    handle
        .link()
        .add(LinkWireguard::new(WG_IFACE).build())
        .execute()
        .await
        .context("creating wireguard link")?;

    let link = get_link_by_name(handle, WG_IFACE)
        .await?
        .context("wireguard link missing after creation")?;

    Ok(link.header.index)
}

async fn configure_addresses(
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

fn configure_peers(config: &ResolvedConfig) -> Result<()> {
    let private_key_path = config.paths.keys.join("server.key");
    let listen_port = config.server.listen_port.to_string();
    let private_key_path = private_key_path
        .to_str()
        .context("server key path not utf-8")?
        .to_string();
    run_wg_command(&[
        "set".to_string(),
        WG_IFACE.to_string(),
        "listen-port".to_string(),
        listen_port,
        "private-key".to_string(),
        private_key_path,
    ])?;

    for peer in &config.peers {
        let peer_dir = config.paths.peers.join(&peer.id);
        let public_key = read_to_string(peer_dir.join("public.key"))?;
        let allowed_ips = peer_allowed_ips(&peer_dir)?;
        if allowed_ips.is_empty() {
            continue;
        }
        let preshared_path = peer_dir
            .join("preshared.key")
            .to_str()
            .context("preshared key path not utf-8")?
            .to_string();
        let allowed_list = allowed_ips.join(",");
        run_wg_command(&[
            "set".to_string(),
            WG_IFACE.to_string(),
            "peer".to_string(),
            public_key.trim().to_string(),
            "preshared-key".to_string(),
            preshared_path,
            "allowed-ips".to_string(),
            allowed_list,
        ])?;
    }

    Ok(())
}

async fn configure_routes(
    handle: &rtnetlink::Handle,
    link_index: u32,
    config: &ResolvedConfig,
) -> Result<()> {
    for peer in &config.peers {
        let peer_dir = config.paths.peers.join(&peer.id);
        for allowed in peer_allowed_ips(&peer_dir)? {
            let ipnet: IpNet = allowed
                .parse()
                .with_context(|| format!("parsing allowed ip {allowed}"))?;
            let res = match ipnet {
                IpNet::V4(v4) => handle
                    .route()
                    .add(
                        RouteMessageBuilder::<std::net::Ipv4Addr>::new()
                            .output_interface(link_index)
                            .destination_prefix(v4.addr(), v4.prefix_len())
                            .build(),
                    )
                    .execute()
                    .await,
                IpNet::V6(v6) => handle
                    .route()
                    .add(
                        RouteMessageBuilder::<std::net::Ipv6Addr>::new()
                            .output_interface(link_index)
                            .destination_prefix(v6.addr(), v6.prefix_len())
                            .build(),
                    )
                    .execute()
                    .await,
            };
            ignore_exists(res).with_context(|| format!("adding route {allowed}"))?;
        }
    }
    Ok(())
}

async fn delete_routes(
    handle: &rtnetlink::Handle,
    link_index: u32,
    config: &ResolvedConfig,
) -> Result<()> {
    for peer in &config.peers {
        let peer_dir = config.paths.peers.join(&peer.id);
        for allowed in peer_allowed_ips(&peer_dir)? {
            let ipnet: IpNet = allowed
                .parse()
                .with_context(|| format!("parsing allowed ip {allowed}"))?;
            let message = match ipnet {
                IpNet::V4(v4) => RouteMessageBuilder::<std::net::Ipv4Addr>::new()
                    .output_interface(link_index)
                    .destination_prefix(v4.addr(), v4.prefix_len())
                    .build(),
                IpNet::V6(v6) => RouteMessageBuilder::<std::net::Ipv6Addr>::new()
                    .output_interface(link_index)
                    .destination_prefix(v6.addr(), v6.prefix_len())
                    .build(),
            };
            let res = handle.route().del(message).execute().await;
            ignore_notfound(res).with_context(|| format!("deleting route {allowed}"))?;
        }
    }
    Ok(())
}

async fn set_link_up(handle: &rtnetlink::Handle, link_index: u32) -> Result<()> {
    handle
        .link()
        .set(LinkUnspec::new_with_index(link_index).up().build())
        .execute()
        .await
        .context("setting wg link up")
}

fn peer_allowed_ips(peer_dir: &Path) -> Result<Vec<String>> {
    let conf_path = peer_dir.join("client.conf");
    if !conf_path.exists() {
        return Ok(Vec::new());
    }
    let text = std::fs::read_to_string(&conf_path)
        .with_context(|| format!("reading {:?}", conf_path))?;
    Ok(extract_addresses(&text))
}

fn extract_addresses(text: &str) -> Vec<String> {
    for line in text.lines() {
        let line = line.trim();
        if let Some(rest) = line.strip_prefix("Address") {
            let parts: Vec<&str> = rest.split('=').collect();
            if parts.len() == 2 {
                return parts[1]
                    .split(',')
                    .map(|item| item.trim().to_string())
                    .filter(|item| !item.is_empty())
                    .collect();
            }
        }
    }
    Vec::new()
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

fn run_wg_command(args: &[String]) -> Result<()> {
    let status = Command::new("wg")
        .args(args)
        .status()
        .with_context(|| format!("running wg {}", args.join(" ")))?;
    if !status.success() {
        anyhow::bail!("wg command failed");
    }
    Ok(())
}

fn best_effort_wg_cleanup(config: &ResolvedConfig) -> Result<()> {
    for peer in &config.peers {
        let peer_dir = config.paths.peers.join(&peer.id);
        let public_key = match read_to_string(peer_dir.join("public.key")) {
            Ok(key) => key,
            Err(err) => {
                eprintln!("wg cleanup: unable to read public key for {}: {err}", peer.id);
                continue;
            }
        };
        let args = [
            "set".to_string(),
            WG_IFACE.to_string(),
            "peer".to_string(),
            public_key.trim().to_string(),
            "remove".to_string(),
        ];
        if let Err(err) = run_wg_command(&args) {
            eprintln!("wg cleanup: failed to remove peer {}: {err}", peer.id);
        }
    }

    let args = [
        "set".to_string(),
        WG_IFACE.to_string(),
        "listen-port".to_string(),
        "0".to_string(),
    ];
    if let Err(err) = run_wg_command(&args) {
        eprintln!("wg cleanup: failed to reset listen port: {err}");
    }

    Ok(())
}

fn read_to_string<P: AsRef<Path>>(path: P) -> Result<String> {
    std::fs::read_to_string(path.as_ref())
        .with_context(|| format!("reading {:?}", path.as_ref()))
}

fn ignore_exists(result: std::result::Result<(), rtnetlink::Error>) -> Result<()> {
    match result {
        Ok(()) => Ok(()),
        Err(rtnetlink::Error::NetlinkError(err))
            if err.code.map(|c| c.get()) == Some(-libc::EEXIST) =>
        {
            Ok(())
        }
        Err(err) => Err(err.into()),
    }
}

fn ignore_notfound(result: std::result::Result<(), rtnetlink::Error>) -> Result<()> {
    match result {
        Ok(()) => Ok(()),
        Err(rtnetlink::Error::NetlinkError(err))
            if err.code.map(|c| c.get()) == Some(-libc::ENOENT) =>
        {
            Ok(())
        }
        Err(err) => Err(err.into()),
    }
}

 
