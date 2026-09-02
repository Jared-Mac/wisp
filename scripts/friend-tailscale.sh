#!/usr/bin/env bash
set -euo pipefail

repo_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
config_file=${XDG_CONFIG_HOME:-${HOME:?HOME is required}/.config}/wisp/friend.env

saved_setting() {
  local name=$1
  [[ -f "$config_file" ]] || return 0
  sed -n "s/^${name}=//p" "$config_file" | tail -n 1
}

explicit_host=${1:-}
explicit_profile=${2:-}
host=${explicit_host:-${WISP_FRIEND_HOST:-$(saved_setting WISP_FRIEND_HOST)}}
profile=${explicit_profile:-${WISP_PROFILE:-$(saved_setting WISP_PROFILE)}}

if [[ -z "$host" || -z "$profile" ]]; then
  echo "No saved Wisp friend identity." >&2
  echo "Configure one with: just friend-config <tailscale-host-or-ip> <Tyler|Jack|Charlie>" >&2
  exit 2
fi
case "$profile" in
  Tyler|Jack|Charlie) ;;
  *)
    echo "profile must be Tyler, Jack, or Charlie; each friend needs a unique profile" >&2
    exit 2
    ;;
esac

if [[ -n "$explicit_host" && -n "$explicit_profile" ]]; then
  "$repo_dir/scripts/configure-friend.sh" "$host" "$profile" >/dev/null
fi

for command_name in tailscale curl wispd wisp-ui; do
  if ! command -v "$command_name" >/dev/null 2>&1; then
    echo "$command_name is missing; run ./scripts/friend-bootstrap-cachyos.sh" >&2
    exit 1
  fi
done

if ! tailscale ping --c 3 --until-direct=false "$host" >/dev/null; then
  backend_state=$(tailscale status --json 2>/dev/null | jq -r '.BackendState // "unknown"')
  echo "Cannot reach Wisp host $host through Tailscale (local state: $backend_state)." >&2
  echo "If local state is Running, ask the host owner to start or re-share the Wisp machine." >&2
  exit 1
fi

server_url="http://$host:8787"
if ! curl --fail --silent --show-error --max-time 5 "$server_url/healthz" >/dev/null; then
  echo "Wisp is not reachable at $server_url; ask the host to run 'just dev-tailscale'" >&2
  exit 1
fi

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

socket_path="${XDG_RUNTIME_DIR:?XDG_RUNTIME_DIR is required}/wisp/wispd.sock"
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

wisp-ui open
echo "Wisp is connected as $profile through $host. Keep this terminal open; Ctrl+C exits."
wait "$daemon_pid"
