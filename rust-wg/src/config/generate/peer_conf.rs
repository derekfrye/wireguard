use crate::config::io::write_atomic;
use crate::config::qr::{print_qr, write_qr_png};
use crate::config::types::{ConfigFile, Paths, Peer};
use anyhow::{Context, Result};
use std::fs;

pub(super) fn generate_peer(
    cfg: &ConfigFile,
    paths: &Paths,
    peer: &Peer,
    ip: std::net::Ipv4Addr,
    ip6: Option<std::net::Ipv6Addr>,
    server_public: &str,
) -> Result<()> {
    let peer_dir = paths.peers.join(&peer.id);
    fs::create_dir_all(&peer_dir).context("creating peer dir")?;

    let keys = super::keys::ensure_peer_keys(&peer_dir)?;

    let mut text = String::new();
    text.push_str("[Interface]\n");
    let mut addresses = vec![format!("{ip}/32")];
    if let Some(v6) = ip6 {
        addresses.push(format!("{v6}/128"));
    }
    text.push_str(&format!("Address = {}\n", addresses.join(", ")));
    text.push_str(&format!("PrivateKey = {}\n", keys.private.trim()));
    if !cfg.network.peer_dns.is_empty() {
        text.push_str(&format!("DNS = {}\n", cfg.network.peer_dns.join(", ")));
    }
    text.push('\n');

    let external = cfg
        .server
        .external_address
        .as_ref()
        .context("external_address must be set to generate peer configs")?;
    text.push_str("[Peer]\n");
    text.push_str(&format!("PublicKey = {}\n", server_public.trim()));
    text.push_str(&format!(
        "Endpoint = {}:{}\n",
        external, cfg.server.listen_port
    ));
    text.push_str(&format!(
        "AllowedIPs = {}\n",
        cfg.network.allowed_ips.join(", ")
    ));

    write_atomic(&peer_dir.join("client.conf"), text.as_bytes())?;

    if cfg.runtime.emit_qr {
        print_qr(&peer_dir.join("client.conf"))?;
        write_qr_png(&peer_dir.join("client.conf"), peer_dir.join("client.png"))?;
    }

    Ok(())
}
