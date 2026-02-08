# Security TODOs

- Peer config files (`client.conf`) are written via `write_atomic` and may inherit a permissive umask; ensure they are written with 0600 or equivalent to avoid local disclosure of private keys.
- Key generation relies on `wg` found on PATH; if PATH is compromised, a malicious `wg` could output predictable keys. Consider hardening how `wg` is located or executed.
