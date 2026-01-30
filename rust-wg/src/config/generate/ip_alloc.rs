use anyhow::{Context, Result};
use std::collections::HashSet;
use std::fs;
use std::path::Path;

pub(super) fn gather_assigned_ips(peers_root: &Path) -> Result<HashSet<String>> {
    let mut used = HashSet::new();
    if !peers_root.exists() {
        return Ok(used);
    }
    for entry in fs::read_dir(peers_root).context("reading peers dir")? {
        let entry = entry?;
        if !entry.file_type()?.is_dir() {
            continue;
        }
        let conf = entry.path().join("client.conf");
        if conf.exists() {
            let text = fs::read_to_string(conf).context("reading client.conf")?;
            for addr in extract_addresses(&text) {
                if addr.contains('.') {
                    used.insert(addr);
                }
            }
        }
    }
    Ok(used)
}

pub(super) fn gather_assigned_ips_v6(peers_root: &Path) -> Result<HashSet<String>> {
    let mut used = HashSet::new();
    if !peers_root.exists() {
        return Ok(used);
    }
    for entry in fs::read_dir(peers_root).context("reading peers dir")? {
        let entry = entry?;
        if !entry.file_type()?.is_dir() {
            continue;
        }
        let conf = entry.path().join("client.conf");
        if conf.exists() {
            let text = fs::read_to_string(conf).context("reading client.conf")?;
            for addr in extract_addresses(&text) {
                if addr.contains(':') {
                    used.insert(addr);
                }
            }
        }
    }
    Ok(used)
}

pub(super) fn extract_addresses(text: &str) -> Vec<String> {
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

pub(super) fn next_available_v4(
    hosts: &mut ipnet::Ipv4AddrRange,
    assigned: &HashSet<String>,
) -> Result<std::net::Ipv4Addr> {
    for addr in hosts {
        let addr_str = addr.to_string();
        if !assigned.contains(&addr_str) {
            return Ok(addr);
        }
    }
    anyhow::bail!("no available IPv4 addresses in subnet")
}

pub(super) fn next_available_v6(
    hosts: &mut ipnet::Ipv6AddrRange,
    assigned: &HashSet<String>,
) -> Result<std::net::Ipv6Addr> {
    for addr in hosts {
        let addr_str = addr.to_string();
        if !assigned.contains(&addr_str) {
            return Ok(addr);
        }
    }
    anyhow::bail!("no available IPv6 addresses in subnet")
}
