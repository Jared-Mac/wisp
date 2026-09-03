#!/usr/bin/env bash
set -euo pipefail

if ! command -v tailscale >/dev/null 2>&1; then
  echo "Tailscale is not installed" >&2
  exit 1
fi

tailscale_ip=$(tailscale ip -4 | head -n 1)
tailscale_dns=$(tailscale status --json | jq -r '.Self.DNSName // empty' | sed 's/\.$//')

echo "Wisp Tailscale host"
echo "  DNS:  ${tailscale_dns:-unavailable}"
echo "  IPv4: $tailscale_ip"
echo "  Friends: just friend-register ${tailscale_dns:-$tailscale_ip} <Tyler|Jack|Charlie>"
echo "Share each one-use invite and the media E2EE key privately; never share the bootstrap token."
