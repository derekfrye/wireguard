#!/usr/bin/env bash
set -euo pipefail

WG_CONTAINER="${WG_CONTAINER:-rust-wg-dev}"
PEER_CONTAINER="${PEER_CONTAINER:-rust-wg-test-peer}"
PEER_ID="${PEER_ID:-}"
WG_LISTEN_PORT="${WG_LISTEN_PORT:-51820}"
WG_ENDPOINT="${WG_ENDPOINT:-}"
WG_ALLOWED_IPS="${WG_ALLOWED_IPS:-10.66.0.0/24}"
WG_PING_TARGET="${WG_PING_TARGET:-10.66.0.1}"
WG_TEST_URL="${WG_TEST_URL:-}"

if [[ -z "$WG_ENDPOINT" ]]; then
    WG_IP="$(podman inspect -f '{{range .NetworkSettings.Networks}}{{.IPAddress}}{{end}}' "$WG_CONTAINER" 2>/dev/null || true)"
    if [[ -n "$WG_IP" ]]; then
        WG_ENDPOINT="${WG_IP}:${WG_LISTEN_PORT}"
    else
        WG_ENDPOINT="${WG_CONTAINER}:${WG_LISTEN_PORT}"
    fi
fi

if [[ -z "$WG_TEST_URL" ]]; then
    WG_TEST_URL="http://${WG_PING_TARGET}/"
fi

if [[ -z "$PEER_ID" ]]; then
    PEER_ID="$(podman exec "$WG_CONTAINER" sh -c 'ls -1 /var/lib/wg/peers 2>/dev/null | head -n 1')"
fi

if [[ -z "$PEER_ID" ]]; then
    echo "no peer found under /var/lib/wg/peers in ${WG_CONTAINER}" >&2
    exit 1
fi

CONF_PATH="/tmp/${PEER_ID}.conf"

cleanup() {
    podman exec "$PEER_CONTAINER" sh -c "wg-quick down '${CONF_PATH}'" >/dev/null 2>&1 || true
}
trap cleanup EXIT

podman exec "$PEER_CONTAINER" sh -c "cp '/var/lib/wg/peers/${PEER_ID}/client.conf' '${CONF_PATH}'"
podman exec "$PEER_CONTAINER" sh -c "sed -i \
    -e 's/^Endpoint.*/Endpoint = ${WG_ENDPOINT}/' \
    -e 's/^AllowedIPs.*/AllowedIPs = ${WG_ALLOWED_IPS}/' \
    -e '/^DNS[[:space:]]*=.*/d' \
    '${CONF_PATH}'"

podman exec "$PEER_CONTAINER" sh -c "wg-quick up '${CONF_PATH}'"
podman exec "$PEER_CONTAINER" sh -c "ping -c 3 '${WG_PING_TARGET}'"
podman exec "$PEER_CONTAINER" sh -c "curl --fail --show-error --silent '${WG_TEST_URL}'"

echo "ok: curl succeeded from ${PEER_CONTAINER} via WireGuard"
