use anyhow::{Context, Result};
use std::io::IsTerminal;
use std::path::{Path, PathBuf};
use std::process::Command;

pub(super) fn print_qr(conf_path: &Path) -> Result<()> {
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

pub(super) fn write_qr_png(conf_path: &Path, output_path: PathBuf) -> Result<()> {
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
