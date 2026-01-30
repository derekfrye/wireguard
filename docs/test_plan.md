# WireGuard Test Plan (dev container)

This plan validates the current `rust-wg` runtime in a Podman container.
Note: peer config generation requires `WG_EXTERNAL_ADDRESS` (or `external_address` in `wg.toml`).

## 1) Build + start

```sh
podman build -t localhost/djf/rust-wg -f examples/Dockerfile .
systemctl --user daemon-reload
systemctl --user start rust-wg-dev.container
```

## 2) Locate generated peer config

Peer configs are stored in the named volume `rust_wg_dev` at `/var/lib/wg/peers/<peer-id>/client.conf`.

Option A: inspect from the container

```sh
podman exec -it rust-wg-dev ls /var/lib/wg/peers
podman exec -it rust-wg-dev cat /var/lib/wg/peers/<peer-id>/client.conf
```

Option B: inspect from the host

```sh
podman volume inspect rust_wg_dev
```

Then open `<Mountpoint>/peers/<peer-id>/client.conf`.

## 3) Sanity checks inside the container

```sh
podman exec -it rust-wg-dev wg show
podman exec -it rust-wg-dev ip link show wg0
podman exec -it rust-wg-dev ip addr show wg0
```

## 4) Test from the host

Copy a `client.conf` to a temp file and adjust two fields for a safe test:

- **Endpoint**: for local testing, set to `127.0.0.1:51821` (or your host LAN IP + 51821).
- **AllowedIPs**: change to `10.66.0.0/24` to avoid routing all traffic.

Then bring it up and verify:

```sh
sudo wg-quick up /path/to/client.conf
ping 10.66.0.1
sudo wg show
```

If you see a handshake and the ping succeeds, the tunnel is working.

## 5) Firewall / port check

Ensure the host allows UDP `51821`. If the handshake never appears, this is the most common blocker.

## 6) Logs / troubleshooting

```sh
journalctl --user -u rust-wg-dev.container
```

If startup logs say `missing CAP_NET_ADMIN` or `kernel module not available`, address host/kernel or container capability issues.
