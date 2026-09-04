#!/usr/bin/env bash
set -euo pipefail

host=${1:-}
profile=${2:-}

if [[ -z "$host" || -z "$profile" ]]; then
  echo "usage: just friend-config <tailscale-host-or-ip> <Tyler|Jack|Charlie>" >&2
  exit 2
fi
case "$profile" in
  Jared|Tyler|Jack|Charlie) ;;
  *)
    echo "profile must be Tyler, Jack, or Charlie; each friend needs a unique profile" >&2
    exit 2
    ;;
esac

script_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
echo "Wisp private alpha requires one-use device enrollment."
if [[ -x "$script_dir/register-friend-device.sh" ]]; then
  exec "$script_dir/register-friend-device.sh" "$host" "$profile"
fi
exec "$script_dir/wisp-friend-register" "$host" "$profile"
