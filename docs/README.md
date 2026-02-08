# Rootless podman WireGuard VPN

This is project sets up WireGuard tunnels within a rootless podman container. Use case: a WireGuard container for your other devices (phone, other computers, etc.) to VPN into your environment. Runs with only the permissions necessary[^1].

Inspired by [linuxserver/wireguard](https://github.com/linuxserver/docker-wireguard), but using `nft` (instead of `iptables`) and programmed in rust rather than shell & `wg-quick` scripts.

## Quickstart

1. Build the image as a non-root user `podman build --tag localhost/djf/rust-wg -f examples/Dockerfile.wireguard`
    a. If you pick a different tag name, adjust the tag name in the next step.
2. Start from the [example systemd quadlet](/examples/rust-wg-dev.container). Configure the variables within; the file is documented.
3. Start the container:
```shell
cp examples/rust-wg-dev.container ~/.config/containers/systemd/
systemctl --user daemon-reload
systemctl --user start rust-wg-dev
# verify it runs
journalctl --user -xeu rust-wg-dev
```
4. Display QR code for a peer
```shell
podman exec -it rust-wg-dev bash
cd /var/lib/wg/peers
peer="$(ls -1 /var/lib/wg/peers | head -n 1)"
qrencode -t utf8 -r /var/lib/wg/peers/$peer/client.conf
```
5. Download WireGuard on your peer (e.g., from iPhone app store) -> create from QR code -> scan prior QR code. Enable it, your phone should now be able to talk to your container's network.


## Overview

At a high level, the crate (which you could run outside of a container if you want to):

- Loads `/etc/wg/wg.toml` (or `WG_CONFIG`) and applies env var overrides.
- Ensures `/var/lib/wg` exists, then generates keys and configs if inputs changed.
- Brings up `wg0`, configures peers/routes, and applies nftables NAT rules.
- Optionally starts CoreDNS.
- Waits for shutdown and tears everything down.

See [design spec](/docs/wg_spec.md) for more.

### Dockerfile.wireguard

Builds a Fedora-based image with `rust-wg` and required tools. The entrypoint is
`rust-wg run`.

Build:

```sh
podman build -t localhost/djf/rust-wg -f examples/Dockerfile.wireguard .
```

### Quadlet

Systemd user unit (quadlet) for a Podman container. It sets common env vars and volume mounts
for `/var/lib/wg`, plus the required capabilities for WireGuard. Has been tested under rootless podman.

Start:

```sh
systemctl --user daemon-reload
systemctl --user start rust-wg-dev.container
```

## CLI commands

- `rust-wg run`: start the runtime (default behavior in the container image).
- `rust-wg generate`: generate configs only, then exit.
- `rust-wg show-peer <peer-id> ...`: placeholder (not implemented yet).

## Configuration sources

### Config file

Default path is `/etc/wg/wg.toml`. Example structure in [design spec](/docs/wg_spec.md).

## Files on disk

Runtime data expected to live under `/var/lib/wg`. Persist this with a named volume (like the provided quadlet) unless you want to regenerate peer keys when you restart the container.

- `keys/` (server keypair)
- `peers/<peer-id>/` (peer keys + `client.conf` + `client.png`)
- `server/server.conf`
- `state/inputs.json`

## Development

### examples/debug_full_tunnel_client_run_as_root.sh

Host-side helper to bring up a WireGuard client for a generated peer config. It uses
`/var/lib/wg/peers/<peer-id>/client.conf` and supports a quick full-tunnel test.

Run as root within a separate container that has the same `/var/lib/wg` mounted as the container running the crate, so the script can read the first peer from that location:

```sh
sudo examples/debug_full_tunnel_client_run_as_root.sh
```

To clean up:

```sh
sudo examples/debug_full_tunnel_client_run_as_root.sh cleanup
```

### examples/setup_wg_and_run_curl_in_peer.sh

Host-side helper that uses a peer test container. It copies a generated peer config from the
WireGuard container, brings up a tunnel inside the peer container, then runs a curl request.

```sh
WG_TEST_URL=http://10.66.0.1:8080/ examples/setup_wg_and_run_curl_in_peer.sh
```

### Test plan

You could follow the [test plan](/docs/test_plan.md) for development/debugging.

[^1]: `NET_ADMIN` and `NET_RAW` permissions; operating within the container’s network namespace, not the host’s.
