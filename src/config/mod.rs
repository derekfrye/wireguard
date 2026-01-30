use anyhow::{Context, Result};
use std::fs;
use std::path::PathBuf;

mod assets;
mod env;
mod generate;
mod inputs;
mod io;
mod peers;
mod qr;
mod types;

pub use types::{
    ConfigFile, NetworkConfig, Paths, PeersConfig, ResolvedConfig, RuntimeConfig, RuntimeConfigFile,
    ServerConfig,
};

pub fn prepare() -> Result<ResolvedConfig> {
    let mut cfg = load_config_file(config_path()?)?;
    env::apply_env_overrides(&mut cfg)?;

    let paths = Paths {
        root: PathBuf::from("/var/lib/wg"),
        keys: PathBuf::from("/var/lib/wg/keys"),
        peers: PathBuf::from("/var/lib/wg/peers"),
        server: PathBuf::from("/var/lib/wg/server"),
        state: PathBuf::from("/var/lib/wg/state"),
    };

    assets::ensure_dirs(&paths)?;

    let peers = peers::resolve_peers(&cfg.peers, &paths)?;
    let runtime = RuntimeConfig {
        enable_coredns: cfg.runtime.enable_coredns,
    };

    let regen_needed = inputs::inputs_changed(&cfg, &paths)? || assets::assets_missing(&paths, &peers)?;
    if regen_needed {
        generate::generate_all(&cfg, &peers, &paths)?;
        inputs::write_inputs_state(&cfg, &paths)?;
    }

    Ok(ResolvedConfig {
        server: cfg.server,
        network: cfg.network,
        peers,
        runtime,
        paths,
    })
}

fn config_path() -> Result<PathBuf> {
    if let Ok(path) = std::env::var("WG_CONFIG")
        && !path.trim().is_empty() {
            return Ok(PathBuf::from(path));
        }
    Ok(PathBuf::from("/etc/wg/wg.toml"))
}

fn load_config_file(path: PathBuf) -> Result<ConfigFile> {
    if !path.exists() {
        return Ok(ConfigFile::default());
    }
    let text = fs::read_to_string(&path).with_context(|| format!("reading {path:?}"))?;
    let cfg: ConfigFile = toml::from_str(&text).context("parsing wg.toml")?;
    Ok(cfg)
}
