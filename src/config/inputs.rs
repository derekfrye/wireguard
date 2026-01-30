use crate::config::Paths;
use crate::config::io::{read_to_string, write_atomic};
use crate::config::types::ConfigFile;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[derive(Debug, Serialize)]
struct InputsSnapshot<'a> {
    server: &'a crate::config::ServerConfig,
    network: &'a crate::config::NetworkConfig,
    peers: &'a crate::config::PeersConfig,
    runtime: &'a crate::config::RuntimeConfigFile,
}

#[derive(Debug, Deserialize, Serialize)]
struct InputsState {
    digest: String,
    inputs: serde_json::Value,
}

pub(super) fn inputs_changed(cfg: &ConfigFile, paths: &Paths) -> Result<bool> {
    let snapshot = InputsSnapshot {
        server: &cfg.server,
        network: &cfg.network,
        peers: &cfg.peers,
        runtime: &cfg.runtime,
    };
    let json = serde_json::to_value(&snapshot).context("serializing inputs")?;
    let digest = hash_json(&json)?;

    let state_path = paths.state.join("inputs.json");
    if !state_path.exists() {
        return Ok(true);
    }
    let text = read_to_string(&state_path)?;
    let state: InputsState = serde_json::from_str(&text).context("parsing inputs.json")?;
    Ok(state.digest != digest)
}

pub(super) fn write_inputs_state(cfg: &ConfigFile, paths: &Paths) -> Result<()> {
    let snapshot = InputsSnapshot {
        server: &cfg.server,
        network: &cfg.network,
        peers: &cfg.peers,
        runtime: &cfg.runtime,
    };
    let json = serde_json::to_value(&snapshot).context("serializing inputs")?;
    let digest = hash_json(&json)?;
    let state = InputsState {
        digest,
        inputs: json,
    };
    let text = serde_json::to_string_pretty(&state).context("serializing inputs.json")?;
    write_atomic(&paths.state.join("inputs.json"), text.as_bytes())?;
    Ok(())
}

fn hash_json(value: &serde_json::Value) -> Result<String> {
    let bytes = serde_json::to_vec(value).context("serializing inputs")?;
    let digest = Sha256::digest(&bytes);
    Ok(hex::encode(digest))
}
