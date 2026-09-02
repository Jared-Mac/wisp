#!/usr/bin/env bash
set -euo pipefail

host=${1:-}
profile=${2:-}

if [[ -z "$host" || -z "$profile" ]]; then
  echo "usage: just friend-config <tailscale-host-or-ip> <Tyler|Jack|Charlie>" >&2
  exit 2
fi
if [[ ! "$host" =~ ^[A-Za-z0-9.-]+$ ]]; then
  echo "host must be a Tailscale DNS name or IP address" >&2
  exit 2
fi
case "$profile" in
  Tyler|Jack|Charlie) ;;
  *)
    echo "profile must be Tyler, Jack, or Charlie; each friend needs a unique profile" >&2
    exit 2
    ;;
esac

config_dir=${XDG_CONFIG_HOME:-${HOME:?HOME is required}/.config}/wisp
config_file="$config_dir/friend.env"
mkdir -p "$config_dir"
temporary=$(mktemp "$config_dir/friend.env.XXXXXX")
trap 'rm -f -- "$temporary"' EXIT
chmod 0600 "$temporary"
printf 'WISP_FRIEND_HOST=%s\nWISP_PROFILE=%s\n' "$host" "$profile" >"$temporary"
mv -f -- "$temporary" "$config_file"
trap - EXIT

echo "Saved Wisp friend profile $profile through $host"
echo "Start it with: just friend"
