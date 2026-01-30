use crate::config::types::{Peer, PeersConfig, Paths};
use anyhow::{Context, Result};
use std::collections::HashSet;
use std::fs;
use uuid::Uuid;

pub(super) fn resolve_peers(peers: &PeersConfig, paths: &Paths) -> Result<Vec<Peer>> {
    if let Some(names) = peers.names.as_ref().filter(|names| !names.is_empty()) {
        let mut seen = HashSet::new();
        let mut peers_out = Vec::new();
        for (idx, name) in names.iter().enumerate() {
            let slug = slugify(name);
            let peer_id = if slug.is_empty() {
                format!("peer-unnamed-{}", idx + 1)
            } else {
                format!("peer-{slug}")
            };
            if !seen.insert(peer_id.clone()) {
                anyhow::bail!("duplicate peer name after slugging: {name}");
            }
            peers_out.push(Peer { id: peer_id });
        }
        return Ok(peers_out);
    }

    resolve_count_peers(peers.count.unwrap_or(0), paths)
}

fn resolve_count_peers(count: usize, paths: &Paths) -> Result<Vec<Peer>> {
    if count == 0 {
        return Ok(Vec::new());
    }
    let mut existing = list_peer_dirs(paths)?;
    existing.sort();
    let mut peers_out = Vec::new();
    let mut seen = HashSet::new();
    for id in existing.into_iter().take(count) {
        if seen.insert(id.clone()) {
            peers_out.push(Peer { id });
        }
    }
    while peers_out.len() < count {
        let id = format!("peer-{}", Uuid::new_v4());
        if seen.insert(id.clone()) {
            peers_out.push(Peer { id });
        }
    }
    Ok(peers_out)
}

fn list_peer_dirs(paths: &Paths) -> Result<Vec<String>> {
    let mut peers = Vec::new();
    if !paths.peers.exists() {
        return Ok(peers);
    }
    for entry in fs::read_dir(&paths.peers).context("reading peers dir")? {
        let entry = entry?;
        if !entry.file_type()?.is_dir() {
            continue;
        }
        if let Some(name) = entry.file_name().to_str() {
            peers.push(name.to_string());
        }
    }
    Ok(peers)
}

fn slugify(input: &str) -> String {
    let mut out = String::new();
    let mut prev_dash = false;
    for ch in input.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_lowercase());
            prev_dash = false;
        } else if !prev_dash {
            out.push('-');
            prev_dash = true;
        }
    }
    out.trim_matches('-').to_string()
}
