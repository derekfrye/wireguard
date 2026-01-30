use anyhow::Result;
use std::process::{Child, Command};

pub fn maybe_start(use_coredns: bool) -> Result<Option<Child>> {
    if !use_coredns {
        return Ok(None);
    }

    // TODO: point to the configured Corefile and add health checks.
    let child = Command::new("coredns").spawn()?;
    Ok(Some(child))
}

pub fn stop(mut child: Child) {
    // TODO: send a graceful signal and wait with timeout.
    let _ = child.kill();
    let _ = child.wait();
}
