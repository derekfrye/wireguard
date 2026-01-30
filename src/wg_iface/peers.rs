use crate::config::ResolvedConfig;
use crate::wg_iface::WG_IFACE;
use anyhow::{Context, Result};
use std::path::Path;
use std::process::Command;

pub(super) fn configure_peers(config: &ResolvedConfig) -> Result<()> {
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

pub(super) fn peer_allowed_ips(peer_dir: &Path) -> Result<Vec<String>> {
    let conf_path = peer_dir.join("client.conf");
    if !conf_path.exists() {
        return Ok(Vec::new());
    }
    let text = std::fs::read_to_string(&conf_path)
        .with_context(|| format!("reading {}", conf_path.display()))?;
    Ok(extract_addresses(&text))
}

pub(super) fn best_effort_wg_cleanup(config: &ResolvedConfig) {
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

fn read_to_string<P: AsRef<Path>>(path: P) -> Result<String> {
    std::fs::read_to_string(path.as_ref())
        .with_context(|| format!("reading {}", path.as_ref().display()))
}
