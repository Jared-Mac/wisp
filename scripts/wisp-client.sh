#!/usr/bin/env bash
set -euo pipefail

script_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
export PATH="$script_dir:$PATH"
config_root=${XDG_CONFIG_HOME:-${HOME:?HOME is required}/.config}/wisp
config_file="$config_root/account.env"
[[ -f "$config_file" ]] || config_file="$config_root/friend.env"
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
server_url=${WISP_SERVER_URL:-$(saved_setting WISP_SERVER_URL)}

if [[ -z "$server_url" && -z "$host" ]] || [[ -z "$profile" ]]; then
  echo "No saved Wisp account." >&2
  exit 2
fi
[[ "$profile" != *$'\n'* && "$profile" != *$'\r'* ]] || {
  echo "saved profile contains an invalid line break" >&2
  exit 2
}
export WISP_PROFILE="$profile"

endpoint_helper="$script_dir/server-endpoint.sh"
[[ -f "$endpoint_helper" ]] || endpoint_helper="$script_dir/wisp-server-endpoint"
source "$endpoint_helper"
if [[ -n "$explicit_host" || -z "$server_url" ]]; then server_url=$host; fi
if [[ "$server_url" == "http://$host:8787" ]]; then server_url=$host; fi
wisp_resolve_endpoint "$server_url"
server_url=$WISP_SELECTED_SERVER_URL
if [[ "$server_url" == https://* ]]; then
  export WISP_REQUIRE_MEDIA_E2EE=true
  export WISP_REQUIRE_CHAT_E2EE=true
fi

if [[ -z "$device_id" || -z "$device_token" ]] || [[ -z "$e2ee_key" && "$server_url" != https://* ]]; then
  echo "This device has not been signed in to Wisp." >&2
  exit 2
fi
export WISP_DEVICE_ID="$device_id"
export WISP_DEVICE_TOKEN="$device_token"
export WISP_E2EE_KEY="$e2ee_key"

# One-server installs migrate in place to the multi-server registry. The
# original account.env remains as a compatibility/rollback copy.
accounts_file="$config_root/accounts.json"
if [[ ! -f "$accounts_file" && -n "$server_url" && -n "$device_id" && -n "$device_token" ]]; then
  for command_name in jq sha256sum; do
    command -v "$command_name" >/dev/null 2>&1 || {
      echo "$command_name is required to migrate the Wisp account registry" >&2
      exit 1
    }
  done
  server_id="server-$(printf '%s' "${server_url%/}" | sha256sum | cut -c1-16)"
  server_name=$(printf '%s' "$server_url" | sed -E 's#^[a-zA-Z]+://##; s#[:/].*$##')
  [[ -n "$server_name" ]] || server_name="Wisp server"
  registry=$(jq -cn \
    --arg id "$server_id" --arg name "$server_name" --arg url "${server_url%/}" \
    --arg profile "$profile" --arg device_id "$device_id" \
    --arg device_token "$device_token" --arg media_key "$e2ee_key" \
    '{version:1,selected_server_id:$id,servers:[{id:$id,name:$name,server_url:$url,profile:$profile,device_id:$device_id,device_token:$device_token,media_key:(if $media_key=="" then null else $media_key end)}]}')
  temporary=$(mktemp "$config_root/.accounts.json.XXXXXX")
  chmod 0600 "$temporary"
  printf '%s\n' "$registry" >"$temporary"
  mv -f -- "$temporary" "$accounts_file"
fi
export WISP_ACCOUNTS_FILE="$accounts_file"

for command_name in wispd wisp-ui; do
  if ! command -v "$command_name" >/dev/null 2>&1; then
    echo "$command_name is missing; reinstall Wisp with ./scripts/install-release.sh" >&2
    exit 1
  fi
done

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
echo "Wisp is running as $profile and will connect to $server_url when available."
echo "Keep this terminal open; Ctrl+C exits."
wait "$daemon_pid"
