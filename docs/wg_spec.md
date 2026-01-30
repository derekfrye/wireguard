# wg Spec (Clean-Room)

This spec defines the wg runtime behavior.

## 1) Configuration model

### 1.1 Config file (preferred)
Path: `/etc/wg/wg.toml`

```toml
[server]
listen_port = 51820
external_address = "vpn.example.com"

[network]
subnet_v4 = "10.66.0.0/24"
subnet_v6 = "fd66::/64"
allowed_ips = ["0.0.0.0/0", "::/0"]
peer_dns = ["10.3.0.100"]

[peers]
count = 3
names = ["laptop", "phone", "tablet"]

[runtime]
enable_coredns = true
emit_qr = true
```

### 1.2 Env var overrides
Supported overrides (all optional):
- `WG_CONFIG=/path/to/wg.toml`
- `WG_LISTEN_PORT`
- `WG_EXTERNAL_ADDRESS`
- `WG_SUBNET_V4`, `WG_SUBNET_V6`
- `WG_ALLOWED_IPS` (comma-delimited)
- `WG_PEER_DNS` (comma-delimited)
- `WG_PEER_COUNT` or `WG_PEER_NAMES` (comma-delimited)
- `WG_ENABLE_COREDNS` (true/false)
- `WG_EMIT_QR` (true/false)

If both `WG_PEER_COUNT` and `WG_PEER_NAMES` are set, `WG_PEER_NAMES` wins.

## 2) Filesystem layout

### 2.1 Runtime directories
Root: `/var/lib/wg`

```
/var/lib/wg/
  keys/
    server.key
    server.pub
  peers/
    <peer-id>/
      private.key
      public.key
      preshared.key
      client.conf
      client.png
  server/
    server.conf
  state/
    inputs.json
```

### 2.2 Templates
No external templates; configs are generated from structured data.

## 3) Peer identity and validation

- If names are provided, peer IDs are `peer-<slug>`, where `<slug>` is a
  lowercase, dash-separated variant of the provided name.
- If a provided name slug is empty after normalization, the ID becomes
  `peer-unnamed-<n>` (1-based index in the provided list).
- If no names are provided and `count` is used, peer IDs are UUID-based:
  `peer-<uuid>`. Existing peer directories are reused before new UUIDs are
  generated.
- If names are provided, peer IDs are `peer-<slug>`, where `<slug>` is a
  lowercase, dash-separated variant of the provided name.
- Reject duplicate names after slugging.

## 4) Address allocation

- The server address is the first usable IP of `subnet_v4` (e.g. `.1`).
- Peer addresses are allocated sequentially from the subnet range, skipping
  already-assigned addresses found in existing `client.conf` files.
- IPv6 addresses are allocated from `subnet_v6` if provided.

## 5) Config generation rules

- Server keys are generated if missing.
- Each peer has a private key, public key, and preshared key.
- Server config includes all peers; peer configs reference server public key.
- Server and peer configs are written atomically (temp file + rename). Key files
  are written directly with `0600` permissions.
- `inputs.json` stores a digest of input settings to decide when regeneration
  is needed.
- `external_address` must be set to generate peer configs; if missing, config
  generation fails with an explicit error.

## 6) Runtime sequence

1) Ensure WireGuard kernel support (netlink probe).
2) Parse config + env overrides.
3) Ensure runtime directories exist.
4) Generate configs if inputs changed.
5) Configure WG interface + routes (netlink).
6) Apply nftables NAT rules (IPv4/IPv6 as applicable).
7) Start CoreDNS if enabled.
8) Wait for signals and teardown in reverse order.

## 7) Logging and UX

- Log messages are plain English.
- Errors are actionable and indicate the next step.
