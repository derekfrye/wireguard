use crate::config::types::{Paths, Peer};
use anyhow::{Context, Result};
use std::fs;

pub(super) fn assets_missing(paths: &Paths, peers: &[Peer]) -> bool {
    if !paths.keys.join("server.key").exists() || !paths.keys.join("server.pub").exists() {
        return true;
    }
    for peer in peers {
        let peer_dir = paths.peers.join(&peer.id);
        if !peer_dir.exists() {
            return true;
        }
        let private_key = peer_dir.join("private.key");
        let public_key = peer_dir.join("public.key");
        let psk = peer_dir.join("preshared.key");
        let client_conf = peer_dir.join("client.conf");
        if !private_key.exists() || !public_key.exists() || !psk.exists() || !client_conf.exists() {
            return true;
        }
    }
    false
}

pub(super) fn ensure_dirs(paths: &Paths) -> Result<()> {
    fs::create_dir_all(&paths.root).context("creating root dir")?;
    fs::create_dir_all(&paths.keys).context("creating keys dir")?;
    fs::create_dir_all(&paths.peers).context("creating peers dir")?;
    fs::create_dir_all(&paths.server).context("creating server dir")?;
    fs::create_dir_all(&paths.state).context("creating state dir")?;
    Ok(())
}
