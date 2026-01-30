use crate::config::io::{read_to_string, run_output, run_output_with_stdin, write_secret};
use crate::config::types::{KeyPair, Paths};
use anyhow::Result;
use std::path::Path;

pub(super) fn ensure_server_keys(paths: &Paths) -> Result<KeyPair> {
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

pub(super) fn ensure_peer_keys(peer_dir: &Path) -> Result<KeyPair> {
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
