use crate::config::types::ConfigFile;
use anyhow::Result;

pub(super) fn apply_env_overrides(cfg: &mut ConfigFile) -> Result<()> {
    if let Some(port) = env_u16("WG_LISTEN_PORT") {
        cfg.server.listen_port = port;
    }
    if let Some(addr) = env_string("WG_EXTERNAL_ADDRESS") {
        cfg.server.external_address = Some(addr);
    }
    if let Some(subnet) = env_string("WG_SUBNET_V4") {
        cfg.network.subnet_v4 = subnet;
    }
    if let Some(subnet) = env_string("WG_SUBNET_V6") {
        cfg.network.subnet_v6 = Some(subnet);
    }
    if let Some(list) = env_list("WG_ALLOWED_IPS") {
        cfg.network.allowed_ips = list;
    }
    if let Some(list) = env_list("WG_PEER_DNS") {
        cfg.network.peer_dns = list;
    }
    if let Some(count) = env_usize("WG_PEER_COUNT") {
        cfg.peers.count = Some(count);
    }
    if let Some(names) = env_list("WG_PEER_NAMES") {
        if !names.is_empty() {
            cfg.peers.names = Some(names);
        }
    }
    if let Some(value) = env_bool("WG_ENABLE_COREDNS") {
        cfg.runtime.enable_coredns = value;
    }
    if let Some(value) = env_bool("WG_EMIT_QR") {
        cfg.runtime.emit_qr = value;
    }
    Ok(())
}

fn env_string(key: &str) -> Option<String> {
    std::env::var(key).ok().filter(|val| !val.is_empty())
}

fn env_u16(key: &str) -> Option<u16> {
    env_string(key).and_then(|val| val.parse().ok())
}

fn env_usize(key: &str) -> Option<usize> {
    env_string(key).and_then(|val| val.parse().ok())
}

fn env_list(key: &str) -> Option<Vec<String>> {
    env_string(key).map(|val| {
        val.split(',')
            .map(|item| item.trim().to_string())
            .filter(|item| !item.is_empty())
            .collect()
    })
}

fn env_bool(key: &str) -> Option<bool> {
    env_string(key).and_then(|val| match val.to_ascii_lowercase().as_str() {
        "true" | "1" | "yes" => Some(true),
        "false" | "0" | "no" => Some(false),
        _ => None,
    })
}
