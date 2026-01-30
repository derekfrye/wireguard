use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ConfigFile {
    #[serde(default)]
    pub server: ServerConfig,
    #[serde(default)]
    pub network: NetworkConfig,
    #[serde(default)]
    pub peers: PeersConfig,
    #[serde(default)]
    pub runtime: RuntimeConfigFile,
}

impl Default for ConfigFile {
    fn default() -> Self {
        Self {
            server: ServerConfig::default(),
            network: NetworkConfig::default(),
            peers: PeersConfig::default(),
            runtime: RuntimeConfigFile::default(),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ServerConfig {
    pub listen_port: u16,
    pub external_address: Option<String>,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            listen_port: 51820,
            external_address: None,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct NetworkConfig {
    pub subnet_v4: String,
    pub subnet_v6: Option<String>,
    pub allowed_ips: Vec<String>,
    pub peer_dns: Vec<String>,
}

impl Default for NetworkConfig {
    fn default() -> Self {
        Self {
            subnet_v4: "10.66.0.0/24".to_string(),
            subnet_v6: None,
            allowed_ips: vec!["0.0.0.0/0".to_string(), "::/0".to_string()],
            peer_dns: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PeersConfig {
    pub count: Option<usize>,
    pub names: Option<Vec<String>>,
}

impl Default for PeersConfig {
    fn default() -> Self {
        Self {
            count: None,
            names: None,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RuntimeConfigFile {
    pub enable_coredns: bool,
    pub emit_qr: bool,
}

impl Default for RuntimeConfigFile {
    fn default() -> Self {
        Self {
            enable_coredns: true,
            emit_qr: true,
        }
    }
}

#[derive(Debug, Clone)]
pub struct RuntimeConfig {
    pub enable_coredns: bool,
}

#[derive(Debug, Clone)]
pub struct Peer {
    pub id: String,
}

#[derive(Debug, Clone)]
pub struct ResolvedConfig {
    pub server: ServerConfig,
    pub network: NetworkConfig,
    pub peers: Vec<Peer>,
    pub runtime: RuntimeConfig,
    pub paths: Paths,
}

#[derive(Debug, Clone)]
pub struct Paths {
    pub root: PathBuf,
    pub keys: PathBuf,
    pub peers: PathBuf,
    pub server: PathBuf,
    pub state: PathBuf,
}

pub(super) struct KeyPair {
    pub private: String,
    pub public: String,
}
