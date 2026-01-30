use anyhow::{Context, Result};
use ipnet::{Ipv4Net, Ipv6Net};
use std::process::{Command, Stdio};

use crate::{config::ResolvedConfig, wg_iface::WG_IFACE};

const TABLE_V4: &str = "wg_nat_v4";
const TABLE_V6: &str = "wg_nat_v6";
const TABLE_FILTER_V4: &str = "wg_filter_v4";
const TABLE_FILTER_V6: &str = "wg_filter_v6";
const CHAIN: &str = "postrouting";
const CHAIN_FWD: &str = "forward";

pub struct NftHandles {
    v4: bool,
    v6: bool,
}

pub fn apply(config: &ResolvedConfig) -> Result<NftHandles> {
    let enable_v4 = config
        .network
        .allowed_ips
        .iter()
        .any(|ip| ip == "0.0.0.0/0");
    let enable_v6 = config
        .network
        .allowed_ips
        .iter()
        .any(|ip| ip == "::/0");

    if (enable_v4 || enable_v6) && !nft_available()? {
        anyhow::bail!("nft binary not found but NAT is required by AllowedIPs");
    }

    if enable_v4 {
        let dev = default_route_dev(false)?;
        let subnet: Ipv4Net = config
            .network
            .subnet_v4
            .parse()
            .context("parsing subnet_v4 for nftables")?;
        apply_nat_v4(&dev, &subnet)?;
        apply_forward_v4()?;
    }

    if enable_v6 {
        let subnet_v6 = config
            .network
            .subnet_v6
            .as_deref()
            .context("subnet_v6 required for ipv6 NAT")?;
        let dev = default_route_dev(true)?;
        let subnet: Ipv6Net = subnet_v6.parse().context("parsing subnet_v6 for nftables")?;
        apply_nat_v6(&dev, &subnet)?;
        apply_forward_v6()?;
    }

    Ok(NftHandles {
        v4: enable_v4,
        v6: enable_v6,
    })
}

pub fn teardown(handles: NftHandles) -> Result<()> {
    if handles.v4 {
        run_nft_command_allow_missing(&["delete", "table", "ip", TABLE_V4])?;
        run_nft_command_allow_missing(&["delete", "table", "ip", TABLE_FILTER_V4])?;
    }
    if handles.v6 {
        run_nft_command_allow_missing(&["delete", "table", "ip6", TABLE_V6])?;
        run_nft_command_allow_missing(&["delete", "table", "ip6", TABLE_FILTER_V6])?;
    }
    Ok(())
}

fn apply_nat_v4(dev: &str, subnet: &Ipv4Net) -> Result<()> {
    run_nft_command_allow_missing(&["delete", "table", "ip", TABLE_V4])?;
    let script = format!(
        "add table ip {table}\n\
         add chain ip {table} {chain} {{ type nat hook postrouting priority 100 ; }}\n\
         add rule ip {table} {chain} oifname \"{dev}\" ip saddr {subnet} masquerade\n",
        table = TABLE_V4,
        chain = CHAIN,
        dev = dev,
        subnet = subnet
    );
    run_nft_script(&script).context("applying ipv4 nftables nat")?;
    Ok(())
}

fn apply_nat_v6(dev: &str, subnet: &Ipv6Net) -> Result<()> {
    run_nft_command_allow_missing(&["delete", "table", "ip6", TABLE_V6])?;
    let script = format!(
        "add table ip6 {table}\n\
         add chain ip6 {table} {chain} {{ type nat hook postrouting priority 100 ; }}\n\
         add rule ip6 {table} {chain} oifname \"{dev}\" ip6 saddr {subnet} masquerade\n",
        table = TABLE_V6,
        chain = CHAIN,
        dev = dev,
        subnet = subnet
    );
    run_nft_script(&script).context("applying ipv6 nftables nat")?;
    Ok(())
}

fn apply_forward_v4() -> Result<()> {
    run_nft_command_allow_missing(&["delete", "table", "ip", TABLE_FILTER_V4])?;
    let script = format!(
        "add table ip {table}\n\
         add chain ip {table} {chain} {{ type filter hook forward priority 0 ; }}\n\
         add rule ip {table} {chain} iifname \"{iface}\" accept\n\
         add rule ip {table} {chain} oifname \"{iface}\" accept\n",
        table = TABLE_FILTER_V4,
        chain = CHAIN_FWD,
        iface = WG_IFACE
    );
    run_nft_script(&script).context("applying ipv4 nftables forward rules")?;
    Ok(())
}

fn apply_forward_v6() -> Result<()> {
    run_nft_command_allow_missing(&["delete", "table", "ip6", TABLE_FILTER_V6])?;
    let script = format!(
        "add table ip6 {table}\n\
         add chain ip6 {table} {chain} {{ type filter hook forward priority 0 ; }}\n\
         add rule ip6 {table} {chain} iifname \"{iface}\" accept\n\
         add rule ip6 {table} {chain} oifname \"{iface}\" accept\n",
        table = TABLE_FILTER_V6,
        chain = CHAIN_FWD,
        iface = WG_IFACE
    );
    run_nft_script(&script).context("applying ipv6 nftables forward rules")?;
    Ok(())
}

fn default_route_dev(is_v6: bool) -> Result<String> {
    let family = if is_v6 { "-6" } else { "-4" };
    let output = Command::new("ip")
        .args([family, "route", "show", "default"])
        .output()
        .with_context(|| format!("running ip {family} route show default"))?;
    if !output.status.success() {
        anyhow::bail!("failed to read default route for {family}");
    }
    let text = String::from_utf8_lossy(&output.stdout);
    parse_dev_from_route(&text).context("parsing default route dev")
}

fn parse_dev_from_route(text: &str) -> Result<String> {
    for line in text.lines() {
        let mut iter = line.split_whitespace();
        while let Some(token) = iter.next() {
            if token == "dev" {
                if let Some(dev) = iter.next() {
                    return Ok(dev.to_string());
                }
            }
        }
    }
    anyhow::bail!("default route device not found")
}

fn run_nft_script(script: &str) -> Result<()> {
    let mut child = Command::new("nft")
        .arg("-f")
        .arg("-")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .context("spawning nft")?;
    if let Some(stdin) = child.stdin.as_mut() {
        use std::io::Write;
        stdin.write_all(script.as_bytes()).context("writing nft script")?;
    }
    let output = child.wait_with_output().context("waiting for nft")?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("nft failed: {}", stderr.trim());
    }
    Ok(())
}

fn run_nft_command_allow_missing(args: &[&str]) -> Result<()> {
    let output = Command::new("nft")
        .args(args)
        .output()
        .with_context(|| format!("running nft {}", args.join(" ")))?;
    if output.status.success() {
        return Ok(());
    }
    let stderr = String::from_utf8_lossy(&output.stderr);
    if stderr.contains("No such file or directory") {
        return Ok(());
    }
    anyhow::bail!("nft command failed: {}", stderr.trim());
}

fn nft_available() -> Result<bool> {
    match Command::new("nft").arg("--version").status() {
        Ok(status) => {
            if status.success() {
                Ok(true)
            } else {
                anyhow::bail!("nft exists but failed to run");
            }
        }
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(err) => Err(err).context("checking nft availability"),
    }
}
