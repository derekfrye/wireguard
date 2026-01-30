#!/usr/bin/env bash
set -euo pipefail

if [[ "${EUID}" -ne 0 ]]; then
  echo "This script must be run as root." >&2
  exit 1
fi

PEERS_DIR="${PEERS_DIR:-/var/lib/wg/peers}"
SERVER_PUB="${SERVER_PUB:-/var/lib/wg/keys/server.pub}"
WG_CLIENT_IFACE="${WG_CLIENT_IFACE:-wg-client}"
WG_CLIENT_PORT="${WG_CLIENT_PORT:-51821}"
WG_SERVER_ENDPOINT="${WG_SERVER_ENDPOINT:-127.0.0.1:51820}"
KEEP_IFACE="${KEEP_IFACE:-0}"
DNS_RULE_PRIORITY="${DNS_RULE_PRIORITY:-100}"
DNS_BYPASS="${DNS_BYPASS:-1}"

dns_v4=()
dns_v6=()

if [[ ! -d "${PEERS_DIR}" ]]; then
  echo "Peers dir not found: ${PEERS_DIR}" >&2
  exit 1
fi

cleanup() {
  if [[ "${KEEP_IFACE}" == "1" ]]; then
    return
  fi
  remove_dns_bypass_rules
  ip -4 rule del not fwmark 51820 table 51820 >/dev/null 2>&1 || true
  ip -4 route del default dev "${WG_CLIENT_IFACE}" table 51820 >/dev/null 2>&1 || true
  ip -6 rule del not fwmark 51820 table 51820 >/dev/null 2>&1 || true
  ip -6 route del default dev "${WG_CLIENT_IFACE}" table 51820 >/dev/null 2>&1 || true
  ip link del "${WG_CLIENT_IFACE}" >/dev/null 2>&1 || true
}
trap cleanup EXIT

load_dns_targets_from_resolv() {
  local ip
  while read -r _ ip; do
    if [[ -z "${ip}" ]]; then
      continue
    fi
    if [[ "${ip}" == *:* ]]; then
      dns_v6+=("${ip}")
    else
      dns_v4+=("${ip}")
    fi
  done < <(awk '/^nameserver[[:space:]]+/ {print $1, $2}' /etc/resolv.conf 2>/dev/null || true)
}

load_dns_targets_from_clientconf() {
  local line ip
  line="$(awk -F= '/^DNS[[:space:]]*=/ {print $2; exit}' "${client_conf}" | tr -d ' ')"
  if [[ -z "${line}" ]]; then
    return
  fi
  IFS=',' read -r -a dns_list <<< "${line}"
  for ip in "${dns_list[@]}"; do
    if [[ -z "${ip}" ]]; then
      continue
    fi
    if [[ "${ip}" == *:* ]]; then
      dns_v6+=("${ip}")
    else
      dns_v4+=("${ip}")
    fi
  done
}

add_dns_bypass_rules() {
  local ip
  if [[ "${DNS_BYPASS}" != "1" ]]; then
    return
  fi
  for ip in "${dns_v4[@]}"; do
    ip -4 rule add to "${ip}/32" lookup main priority "${DNS_RULE_PRIORITY}" >/dev/null 2>&1 || true
  done
  for ip in "${dns_v6[@]}"; do
    ip -6 rule add to "${ip}/128" lookup main priority "${DNS_RULE_PRIORITY}" >/dev/null 2>&1 || true
  done
}

remove_dns_bypass_rules() {
  local ip
  if [[ "${#dns_v4[@]}" -eq 0 && "${#dns_v6[@]}" -eq 0 ]]; then
    load_dns_targets_from_resolv
  fi
  for ip in "${dns_v4[@]}"; do
    ip -4 rule del to "${ip}/32" lookup main priority "${DNS_RULE_PRIORITY}" >/dev/null 2>&1 || true
  done
  for ip in "${dns_v6[@]}"; do
    ip -6 rule del to "${ip}/128" lookup main priority "${DNS_RULE_PRIORITY}" >/dev/null 2>&1 || true
  done
}

action="${1:-}"
if [[ "${action}" == "cleanup" ]]; then
  KEEP_IFACE=0
  load_dns_targets_from_resolv
  cleanup
  exit 0
fi
if [[ "${action}" == "nodnsbypass" ]]; then
  DNS_BYPASS=0
  action=""
fi

peer_id="${action}"
if [[ -z "${peer_id}" ]]; then
  peer_id="$(ls -1 "${PEERS_DIR}" | head -n 1 || true)"
  if [[ -z "${peer_id}" ]]; then
    echo "No peers found in ${PEERS_DIR}" >&2
    exit 1
  fi
  echo "No peer-id supplied; using ${peer_id}"
fi

peer_dir="${PEERS_DIR}/${peer_id}"
client_conf="${peer_dir}/client.conf"
client_priv="${peer_dir}/private.key"
client_psk="${peer_dir}/preshared.key"

for path in "${client_conf}" "${client_priv}" "${client_psk}" "${SERVER_PUB}"; do
  if [[ ! -f "${path}" ]]; then
    echo "Missing required file: ${path}" >&2
    exit 1
  fi
done
load_dns_targets_from_clientconf
if [[ "${#dns_v4[@]}" -eq 0 && "${#dns_v6[@]}" -eq 0 ]]; then
  load_dns_targets_from_resolv
fi

# Ensure clean slate
ip link del "${WG_CLIENT_IFACE}" >/dev/null 2>&1 || true

client_ip_line="$(awk -F= '/^Address[[:space:]]*=/ {print $2; exit}' "${client_conf}" | tr -d ' ')"
if [[ -z "${client_ip_line}" ]]; then
  echo "Could not parse Address from ${client_conf}" >&2
  exit 1
fi

client_v4="$(printf '%s' "${client_ip_line}" | cut -d',' -f1)"
client_v6="$(printf '%s' "${client_ip_line}" | cut -d',' -f2-)"

ip link add dev "${WG_CLIENT_IFACE}" type wireguard
wg set "${WG_CLIENT_IFACE}" \
  private-key <(cat "${client_priv}") \
  listen-port "${WG_CLIENT_PORT}" \
  peer "$(cat "${SERVER_PUB}")" \
  preshared-key <(cat "${client_psk}") \
  allowed-ips 0.0.0.0/0,::/0 \
  endpoint "${WG_SERVER_ENDPOINT}"

ip -4 addr add "${client_v4}" dev "${WG_CLIENT_IFACE}"
if [[ -n "${client_v6}" ]]; then
  ip -6 addr add "$(printf '%s' "${client_v6}" | tr -d ' ')" dev "${WG_CLIENT_IFACE}" || true
fi
ip link set "${WG_CLIENT_IFACE}" up

ip -4 rule add not fwmark 51820 table 51820
ip -4 route add default dev "${WG_CLIENT_IFACE}" table 51820
ip -6 rule add not fwmark 51820 table 51820
ip -6 route add default dev "${WG_CLIENT_IFACE}" table 51820
add_dns_bypass_rules

echo "wg-client up. Testing tunnel..."
ping -c 3 10.66.0.1
wg show
curl -4 -v --max-time 5 https://ifconfig.me || curl -4 -v --max-time 5 https://api.ipify.org
