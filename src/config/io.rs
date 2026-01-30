use anyhow::{Context, Result};
use std::fs;
use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};

pub(super) fn read_to_string<P: AsRef<Path>>(path: P) -> Result<String> {
    fs::read_to_string(path.as_ref()).with_context(|| format!("reading {:?}", path.as_ref()))
}

pub(super) fn write_atomic(path: &Path, data: &[u8]) -> Result<()> {
    let tmp = path.with_extension("tmp");
    {
        let mut file = fs::OpenOptions::new()
            .create(true)
            .truncate(true)
            .write(true)
            .open(&tmp)
            .with_context(|| format!("writing {tmp:?}"))?;
        file.write_all(data).context("writing temp file")?;
    }
    fs::rename(&tmp, path).with_context(|| format!("renaming {tmp:?} -> {path:?}"))?;
    Ok(())
}

pub(super) fn run_output(cmd: &str, args: &[&str]) -> Result<String> {
    let output = Command::new(cmd)
        .args(args)
        .output()
        .with_context(|| format!("running {cmd}"))?;
    if !output.status.success() {
        anyhow::bail!("command {cmd} failed");
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

pub(super) fn run_output_with_stdin(cmd: &str, args: &[&str], input: &str) -> Result<String> {
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

pub(super) fn write_secret<P: AsRef<Path>>(path: P, data: &str) -> Result<()> {
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
