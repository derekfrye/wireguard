# WireGuard Test Plan (dev container)

This plan validates the current `rust-wg` runtime in a Podman container.
Note: peer config generation requires `WG_EXTERNAL_ADDRESS` (or `external_address` in `wg.toml`). The example quadlet sets `WG_EXTERNAL_ADDRESS` for you; set it manually if you don't use the quadlet.

## 1) Build + start the WireGuard container

```sh
podman build -t localhost/djf/rust-wg -f examples/Dockerfile.wireguard .
systemctl --user daemon-reload
systemctl --user start rust-wg-dev.container
```


## 2) Test from a peer container

Build and start a peer test container (joins the same network as the WireGuard container):

```sh
podman build -t localhost/djf/rust-wg-test-peer -f examples/Dockerfile.test_peer .
cp examples/test_peer.container ~/.config/containers/systemd/
systemctl --user daemon-reload
systemctl --user start test_peer.container
```

Then run the host-side helper to bring up the WireGuard tunnel inside the peer container and curl a URL that is reachable over the tunnel:

```sh
WG_TEST_URL=http://10.66.0.1:8080/ examples/setup_wg_and_run_curl_in_peer.sh
```

If the curl succeeds, the integration path is working. You can override `WG_ALLOWED_IPS`,
`WG_PING_TARGET`, or `PEER_ID` in the script environment as needed. Set `WG_ENDPOINT` only
if your network disables container DNS.


## Logs / troubleshooting

```sh
journalctl --user -u rust-wg-dev.container
```

If startup logs say `missing CAP_NET_ADMIN` or `kernel module not available`, address host/kernel or container capability issues.
