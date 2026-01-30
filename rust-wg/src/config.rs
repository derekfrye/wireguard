use anyhow::{Context, Result};
use ipnet::{Ipv4Net, Ipv6Net};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashSet;
use std::fs;
use std::io::{IsTerminal, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use uuid::Uuid;

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

#[derive(Debug, Serialize)]
struct InputsSnapshot<'a> {
    server: &'a ServerConfig,
    network: &'a NetworkConfig,
    peers: &'a PeersConfig,
    runtime: &'a RuntimeConfigFile,
}

#[derive(Debug, Deserialize, Serialize)]
struct InputsState {
    digest: String,
    inputs: serde_json::Value,
}

pub fn prepare() -> Result<ResolvedConfig> {
    let mut cfg = load_config_file(config_path()?)?;
    apply_env_overrides(&mut cfg)?;

    let paths = Paths {
        root: PathBuf::from("/var/lib/wg"),
        keys: PathBuf::from("/var/lib/wg/keys"),
        peers: PathBuf::from("/var/lib/wg/peers"),
        server: PathBuf::from("/var/lib/wg/server"),
        state: PathBuf::from("/var/lib/wg/state"),
    };

    ensure_dirs(&paths)?;

    let peers = resolve_peers(&cfg.peers, &paths)?;
    let runtime = RuntimeConfig {
        enable_coredns: cfg.runtime.enable_coredns,
    };

    let regen_needed = inputs_changed(&cfg, &paths)? || assets_missing(&paths, &peers)?;
    if regen_needed {
        generate_all(&cfg, &peers, &paths)?;
        write_inputs_state(&cfg, &paths)?;
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
    if let Ok(path) = std::env::var("WG_CONFIG") {
        if !path.trim().is_empty() {
            return Ok(PathBuf::from(path));
        }
    }
    Ok(PathBuf::from("/etc/wg/wg.toml"))
}

fn load_config_file(path: PathBuf) -> Result<ConfigFile> {
    if !path.exists() {
        return Ok(ConfigFile::default());
    }
    let text = fs::read_to_string(&path).with_context(|| format!("reading {:?}", path))?;
    let cfg: ConfigFile = toml::from_str(&text).context("parsing wg.toml")?;
    Ok(cfg)
}

fn apply_env_overrides(cfg: &mut ConfigFile) -> Result<()> {
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

fn resolve_peers(peers: &PeersConfig, paths: &Paths) -> Result<Vec<Peer>> {
    if let Some(names) = peers.names.as_ref().filter(|names| !names.is_empty()) {
        let mut seen = HashSet::new();
        let mut peers_out = Vec::new();
        for (idx, name) in names.iter().enumerate() {
            let slug = slugify(name);
            let peer_id = if slug.is_empty() {
                format!("peer-unnamed-{}", idx + 1)
            } else {
                format!("peer-{}", slug)
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

fn assets_missing(paths: &Paths, peers: &[Peer]) -> Result<bool> {
    if !paths.keys.join("server.key").exists() || !paths.keys.join("server.pub").exists() {
        return Ok(true);
    }
    for peer in peers {
        let peer_dir = paths.peers.join(&peer.id);
        if !peer_dir.exists() {
            return Ok(true);
        }
        let private_key = peer_dir.join("private.key");
        let public_key = peer_dir.join("public.key");
        let psk = peer_dir.join("preshared.key");
        let client_conf = peer_dir.join("client.conf");
        if !private_key.exists() || !public_key.exists() || !psk.exists() || !client_conf.exists() {
            return Ok(true);
        }
    }
    Ok(false)
}

fn ensure_dirs(paths: &Paths) -> Result<()> {
    fs::create_dir_all(&paths.root).context("creating root dir")?;
    fs::create_dir_all(&paths.keys).context("creating keys dir")?;
    fs::create_dir_all(&paths.peers).context("creating peers dir")?;
    fs::create_dir_all(&paths.server).context("creating server dir")?;
    fs::create_dir_all(&paths.state).context("creating state dir")?;
    Ok(())
}

fn inputs_changed(cfg: &ConfigFile, paths: &Paths) -> Result<bool> {
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
    let text = fs::read_to_string(&state_path).context("reading inputs.json")?;
    let state: InputsState = serde_json::from_str(&text).context("parsing inputs.json")?;
    Ok(state.digest != digest)
}

fn write_inputs_state(cfg: &ConfigFile, paths: &Paths) -> Result<()> {
    let snapshot = InputsSnapshot {
        server: &cfg.server,
        network: &cfg.network,
        peers: &cfg.peers,
        runtime: &cfg.runtime,
    };
    let json = serde_json::to_value(&snapshot).context("serializing inputs")?;
    let digest = hash_json(&json)?;
    let state = InputsState { digest, inputs: json };
    let text = serde_json::to_string_pretty(&state).context("serializing inputs.json")?;
    write_atomic(&paths.state.join("inputs.json"), text.as_bytes())?;
    Ok(())
}

fn generate_all(cfg: &ConfigFile, peers: &[Peer], paths: &Paths) -> Result<()> {
    let server_keys = ensure_server_keys(paths)?;

    let v4_net: Ipv4Net = cfg
        .network
        .subnet_v4
        .parse()
        .context("parsing subnet_v4")?;
    let v6_net: Option<Ipv6Net> = match cfg.network.subnet_v6.as_deref() {
        Some(value) => Some(value.parse().context("parsing subnet_v6")?),
        None => None,
    };

    let mut assigned_v4 = gather_assigned_ips(&paths.peers)?;
    let mut assigned_v6 = gather_assigned_ips_v6(&paths.peers)?;

    let mut v4_hosts = v4_net.hosts();
    let server_v4 = v4_hosts
        .next()
        .context("subnet_v4 has no usable host address")?;
    assigned_v4.insert(server_v4.to_string());

    let server_v6 = if let Some(net) = v6_net.as_ref() {
        let mut hosts = net.hosts();
        let addr = hosts
            .next()
            .context("subnet_v6 has no usable host address")?;
        assigned_v6.insert(addr.to_string());
        Some(addr)
    } else {
        None
    };

    let mut peer_ips = Vec::new();
    for _ in peers {
        let ip = next_available_v4(&mut v4_hosts, &assigned_v4)?;
        assigned_v4.insert(ip.to_string());
        let ip6 = if let Some(net) = v6_net.as_ref() {
            let mut hosts = net.hosts();
            let ip6 = next_available_v6(&mut hosts, &assigned_v6)?;
            assigned_v6.insert(ip6.to_string());
            Some(ip6)
        } else {
            None
        };
        peer_ips.push((ip, ip6));
    }

    for peer in peers {
        let peer_dir = paths.peers.join(&peer.id);
        fs::create_dir_all(&peer_dir).context("creating peer dir")?;
        let _ = ensure_peer_keys(&peer_dir)?;
    }

    write_server_conf(
        cfg,
        paths,
        &server_keys.private,
        server_v4,
        server_v6,
        peers,
        &peer_ips,
    )?;

    for (peer, (ip, ip6)) in peers.iter().zip(peer_ips.into_iter()) {
        generate_peer(cfg, paths, peer, ip, ip6, &server_keys.public)?;
    }

    Ok(())
}

fn write_server_conf(
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

fn generate_peer(
    cfg: &ConfigFile,
    paths: &Paths,
    peer: &Peer,
    ip: std::net::Ipv4Addr,
    ip6: Option<std::net::Ipv6Addr>,
    server_public: &str,
) -> Result<()> {
    let peer_dir = paths.peers.join(&peer.id);
    fs::create_dir_all(&peer_dir).context("creating peer dir")?;

    let keys = ensure_peer_keys(&peer_dir)?;

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

fn ensure_server_keys(paths: &Paths) -> Result<KeyPair> {
    let private_path = paths.keys.join("server.key");
    let public_path = paths.keys.join("server.pub");
    if private_path.exists() && public_path.exists() {
        return Ok(KeyPair {
            private: read_to_string(private_path)?,
            public: read_to_string(public_path)?,
        });
    }
    let private = run_output("wg", &["genkey"])?;
    write_secret(&private_path, &private)?;
    let public = run_output_with_stdin("wg", &["pubkey"], &private)?;
    write_secret(&public_path, &public)?;
    Ok(KeyPair { private, public })
}

fn ensure_peer_keys(peer_dir: &Path) -> Result<KeyPair> {
    let private_path = peer_dir.join("private.key");
    let public_path = peer_dir.join("public.key");
    let psk_path = peer_dir.join("preshared.key");

    let private = if private_path.exists() {
        read_to_string(&private_path)?
    } else {
        let key = run_output("wg", &["genkey"])?;
        write_secret(&private_path, &key)?;
        key
    };

    let public = if public_path.exists() {
        read_to_string(&public_path)?
    } else {
        let key = run_output_with_stdin("wg", &["pubkey"], &private)?;
        write_secret(&public_path, &key)?;
        key
    };

    if !psk_path.exists() {
        let psk = run_output("wg", &["genpsk"])?;
        write_secret(&psk_path, &psk)?;
    }

    Ok(KeyPair { private, public })
}

fn gather_assigned_ips(peers_root: &Path) -> Result<HashSet<String>> {
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

fn gather_assigned_ips_v6(peers_root: &Path) -> Result<HashSet<String>> {
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

fn next_available_v4(
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

fn next_available_v6(
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

fn hash_json(value: &serde_json::Value) -> Result<String> {
    let bytes = serde_json::to_vec(value).context("serializing inputs")?;
    let digest = Sha256::digest(&bytes);
    Ok(hex::encode(digest))
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

fn read_to_string<P: AsRef<Path>>(path: P) -> Result<String> {
    fs::read_to_string(path.as_ref()).with_context(|| format!("reading {:?}", path.as_ref()))
}

fn write_atomic(path: &Path, data: &[u8]) -> Result<()> {
    let tmp = path.with_extension("tmp");
    {
        let mut file = fs::OpenOptions::new()
            .create(true)
            .truncate(true)
            .write(true)
            .open(&tmp)
            .with_context(|| format!("writing {:?}", tmp))?;
        file.write_all(data).context("writing temp file")?;
    }
    fs::rename(&tmp, path).with_context(|| format!("renaming {:?} -> {:?}", tmp, path))?;
    Ok(())
}

fn run_output(cmd: &str, args: &[&str]) -> Result<String> {
    let output = Command::new(cmd)
        .args(args)
        .output()
        .with_context(|| format!("running {cmd}"))?;
    if !output.status.success() {
        anyhow::bail!("command {cmd} failed");
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

fn run_output_with_stdin(cmd: &str, args: &[&str], input: &str) -> Result<String> {
    let mut child = Command::new(cmd)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .with_context(|| format!("spawning {cmd}"))?;
    if let Some(stdin) = child.stdin.as_mut() {
        stdin.write_all(input.as_bytes()).context("writing stdin")?;
    }
    let output = child.wait_with_output().context("waiting for command")?;
    if !output.status.success() {
        anyhow::bail!("command {cmd} failed");
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

fn write_secret<P: AsRef<Path>>(path: P, data: &str) -> Result<()> {
    use std::os::unix::fs::OpenOptionsExt;

    let mut file = fs::OpenOptions::new()
        .create(true)
        .truncate(true)
        .write(true)
        .mode(0o600)
        .open(path.as_ref())
        .context("opening secret file")?;
    file.write_all(data.as_bytes())
        .context("writing secret file")?;
    Ok(())
}

fn print_qr(conf_path: &Path) -> Result<()> {
    let format = if std::io::stdout().is_terminal() {
        "ansiutf8"
    } else {
        "utf8"
    };
    let status = match Command::new("qrencode")
        .args(["-t", format, "-r"])
        .arg(conf_path)
        .status()
    {
        Ok(status) => status,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            eprintln!("qr: qrencode not found; skipping terminal output");
            return Ok(());
        }
        Err(err) => return Err(err).context("running qrencode"),
    };
    if !status.success() {
        anyhow::bail!("qrencode failed");
    }
    Ok(())
}

fn write_qr_png(conf_path: &Path, output_path: PathBuf) -> Result<()> {
    let status = match Command::new("qrencode")
        .args(["-o"])
        .arg(output_path)
        .arg("-r")
        .arg(conf_path)
        .status()
    {
        Ok(status) => status,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            eprintln!("qr: qrencode not found; skipping png output");
            return Ok(());
        }
        Err(err) => return Err(err).context("writing qr png"),
    };
    if !status.success() {
        anyhow::bail!("qrencode png failed");
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

struct KeyPair {
    private: String,
    public: String,
}
