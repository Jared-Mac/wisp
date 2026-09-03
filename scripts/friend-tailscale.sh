#!/usr/bin/env bash
set -euo pipefail

script_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
export PATH="$script_dir:$PATH"
config_file=${XDG_CONFIG_HOME:-${HOME:?HOME is required}/.config}/wisp/friend.env
socket_path=${XDG_RUNTIME_DIR:?XDG_RUNTIME_DIR is required}/wisp/wispd.sock

if command -v wisp-ui >/dev/null 2>&1 \
  && [[ -S "$socket_path" ]] \
  && pgrep -u "$(id -u)" -x wispd >/dev/null 2>&1; then
  wisp-ui open
  exit 0
fi

saved_setting() {
  local name=$1
  [[ -f "$config_file" ]] || return 0
  sed -n "s/^${name}=//p" "$config_file" | tail -n 1
}

explicit_host=${1:-}
explicit_profile=${2:-}
host=${explicit_host:-${WISP_FRIEND_HOST:-$(saved_setting WISP_FRIEND_HOST)}}
profile=${explicit_profile:-${WISP_PROFILE:-$(saved_setting WISP_PROFILE)}}
device_id=${WISP_DEVICE_ID:-$(saved_setting WISP_DEVICE_ID)}
device_token=${WISP_DEVICE_TOKEN:-$(saved_setting WISP_DEVICE_TOKEN)}
e2ee_key=${WISP_E2EE_KEY:-$(saved_setting WISP_E2EE_KEY)}

if [[ -z "$host" || -z "$profile" ]]; then
  echo "No saved Wisp friend identity." >&2
  echo "Enroll with: just friend-register <tailscale-host-or-ip> <Tyler|Jack|Charlie>" >&2
  exit 2
fi
case "$profile" in
  Tyler|Jack|Charlie) ;;
  *)
    echo "profile must be Tyler, Jack, or Charlie; each friend needs a unique profile" >&2
    exit 2
    ;;
esac
export WISP_PROFILE="$profile"

if [[ -z "$device_id" || -z "$device_token" || -z "$e2ee_key" ]]; then
  echo "This device has not been enrolled for private-alpha access." >&2
  echo "Ask the host for a one-use invite and private media key, then run:" >&2
  echo "  just friend-register $host $profile" >&2
  exit 2
fi
export WISP_DEVICE_ID="$device_id"
export WISP_DEVICE_TOKEN="$device_token"
export WISP_E2EE_KEY="$e2ee_key"

for command_name in wispd wisp-ui; do
  if ! command -v "$command_name" >/dev/null 2>&1; then
    echo "$command_name is missing; run ./scripts/friend-bootstrap-cachyos.sh" >&2
    exit 1
  fi
done

server_url="http://$host:8787"
daemon_pid=""
cleanup() {
  trap - EXIT INT TERM
  [[ -z "$daemon_pid" ]] || kill "$daemon_pid" 2>/dev/null || true
  [[ -z "$daemon_pid" ]] || wait "$daemon_pid" 2>/dev/null || true
  wisp-ui quit >/dev/null 2>&1 || true
}
trap cleanup EXIT INT TERM

WISP_SERVER_URL="$server_url" wispd --profile "$profile" &
daemon_pid=$!

for _ in $(seq 1 100); do
  [[ -S "$socket_path" ]] && break
  if ! kill -0 "$daemon_pid" 2>/dev/null; then
    wait "$daemon_pid"
  fi
  sleep 0.1
done
if [[ ! -S "$socket_path" ]]; then
  echo "wispd did not create $socket_path" >&2
  exit 1
fi

wisp-ui app open
echo "Wisp is running as $profile and will connect through $host when available."
echo "Keep this terminal open; Ctrl+C exits."
wait "$daemon_pid"
