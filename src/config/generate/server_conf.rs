use crate::config::io::{read_to_string, write_atomic};
use crate::config::types::{ConfigFile, Paths, Peer};
use anyhow::Result;

pub(super) fn write_server_conf(
    cfg: &ConfigFile,
    paths: &Paths,
    private_key: &str,
    server_v4: std::net::Ipv4Addr,
    server_v6: Option<std::net::Ipv6Addr>,
    peers: &[Peer],
    peer_ips: &[(std::net::Ipv4Addr, Option<std::net::Ipv6Addr>)],
) -> Result<()> {
    let mut text = String::new();
    text.push_str("[Interface]\n");
    let mut addresses = vec![format!("{server_v4}/32")];
    if let Some(v6) = server_v6 {
        addresses.push(format!("{v6}/128"));
    }
    text.push_str(&format!("Address = {}\n", addresses.join(", ")));
    text.push_str(&format!("ListenPort = {}\n", cfg.server.listen_port));
    text.push_str(&format!("PrivateKey = {}\n\n", private_key.trim()));

    for (peer, (ip, ip6)) in peers.iter().zip(peer_ips.iter()) {
        let public_key = read_to_string(paths.peers.join(&peer.id).join("public.key"))?;
        let psk = read_to_string(paths.peers.join(&peer.id).join("preshared.key"))?;

        text.push_str("[Peer]\n");
        text.push_str(&format!("PublicKey = {}\n", public_key.trim()));
        text.push_str(&format!("PresharedKey = {}\n", psk.trim()));
        let mut allowed = vec![format!("{ip}/32")];
        if let Some(v6) = ip6 {
            allowed.push(format!("{v6}/128"));
        }
        text.push_str(&format!("AllowedIPs = {}\n\n", allowed.join(", ")));
    }

    write_atomic(&paths.server.join("server.conf"), text.as_bytes())?;
    Ok(())
}
