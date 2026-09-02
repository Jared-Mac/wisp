#!/usr/bin/env bash
set -euo pipefail

repo_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
template="$repo_dir/infra/local/livekit-tailscale.yaml.template"

for command_name in tailscale jq openssl sed; do
  if ! command -v "$command_name" >/dev/null 2>&1; then
    echo "$command_name is required for the Tailscale test host" >&2
    exit 1
  fi
done

backend_state=$(tailscale status --json 2>/dev/null | jq -r '.BackendState // empty')
if [[ "$backend_state" != "Running" ]]; then
  echo "Tailscale is not connected; run: sudo tailscale up" >&2
  exit 1
fi

tailscale_ip=${WISP_TAILSCALE_IP:-$(tailscale ip -4 | head -n 1)}
if [[ ! "$tailscale_ip" =~ ^100\.[0-9]{1,3}\.[0-9]{1,3}\.[0-9]{1,3}$ ]]; then
  echo "Could not determine a valid Tailscale IPv4 address" >&2
  exit 1
fi

livekit_api_key=wisp-tailscale-dev
livekit_api_secret=$(openssl rand -hex 32)
runtime_root=${XDG_RUNTIME_DIR:?XDG_RUNTIME_DIR is required}
runtime_dir="$runtime_root/wisp"
mkdir -p "$runtime_dir"
livekit_config=$(mktemp "$runtime_dir/livekit-tailscale.XXXXXX.yaml")

cleanup() {
  rm -f -- "$livekit_config"
}
trap cleanup EXIT INT TERM

sed \
  -e "s/__TAILSCALE_IP__/$tailscale_ip/g" \
  -e "s/__LIVEKIT_API_KEY__/$livekit_api_key/g" \
  -e "s/__LIVEKIT_API_SECRET__/$livekit_api_secret/g" \
  "$template" >"$livekit_config"
chmod 0600 "$livekit_config"

tailscale_dns=$(tailscale status --json | jq -r '.Self.DNSName // empty' | sed 's/\.$//')
friend_host=${tailscale_dns:-$tailscale_ip}

echo "Starting Wisp's trusted Tailscale test host"
echo "  Wisp:   http://$friend_host:8787"
echo "  LiveKit: ws://$friend_host:7880"
echo "  Media:  UDP $tailscale_ip:7882 (TCP fallback 7881)"
echo "Friends can run: just friend $friend_host Tyler"

WISP_SERVER_ADDR="$tailscale_ip:8787" \
WISP_SERVER_URL="http://$tailscale_ip:8787" \
WISP_LIVEKIT_URL="ws://$tailscale_ip:7880" \
WISP_LIVEKIT_API_KEY="$livekit_api_key" \
WISP_LIVEKIT_API_SECRET="$livekit_api_secret" \
WISP_LIVEKIT_CONFIG="$livekit_config" \
  "$repo_dir/scripts/dev.sh"
