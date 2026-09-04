#!/usr/bin/env bash
set -euo pipefail

host=${1:-}
profile=${2:-}

if [[ -z "$host" || -z "$profile" ]]; then
  echo "usage: just friend-register <tailscale-host-or-ip> <Tyler|Jack|Charlie>" >&2
  exit 2
fi
script_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
endpoint_helper="$script_dir/server-endpoint.sh"
[[ -f "$endpoint_helper" ]] || endpoint_helper="$script_dir/wisp-server-endpoint"
source "$endpoint_helper"
wisp_resolve_endpoint "$host"
server_url=$WISP_SELECTED_SERVER_URL
host=$WISP_SELECTED_SERVER_HOST
case "$profile" in
  Jared|Tyler|Jack|Charlie) ;;
  *) echo "profile must be Tyler, Jack, or Charlie" >&2; exit 2 ;;
esac
for command_name in curl jq; do
  command -v "$command_name" >/dev/null 2>&1 || {
    echo "$command_name is required" >&2
    exit 1
  }
done

invite_code=${WISP_INVITE_CODE:-}
if [[ -z "$invite_code" ]]; then
  read -rsp "One-use Wisp invite: " invite_code
  echo
fi
e2ee_key=${WISP_E2EE_KEY:-}
if [[ -z "$e2ee_key" ]]; then
  if [[ "$server_url" == https://* ]]; then
    read -rsp "Private media key (Enter to configure encrypted chat first; voice stays blocked): " e2ee_key
  else
    read -rsp "Private media key: " e2ee_key
  fi
  echo
fi
if [[ -z "$e2ee_key" && "$server_url" != https://* ]]; then
  echo "private media key must contain at least 16 characters" >&2
  exit 2
fi
if [[ -n "$e2ee_key" && ! "$e2ee_key" =~ ^[A-Za-z0-9_-]{16,128}$ ]]; then
  echo "private media key must use 16–128 letters, digits, '_' or '-'" >&2
  exit 2
fi

device_name=${HOSTNAME:-CachyOS device}
response=$(printf '%s\n%s' "$invite_code" "$device_name" | jq -Rs \
  'split("\n") | {invite_code:.[0], device_name:.[1], protocol_version:1}' \
  | curl --silent --show-error --fail-with-body \
      -H 'content-type: application/json' \
      --data-binary @- \
      "$server_url/v1/devices/register")

device_id=$(jq -er '.device_id' <<<"$response")
device_token=$(jq -er '.device_token' <<<"$response")
returned_profile=$(jq -er '.user.display_name' <<<"$response")
if [[ "$returned_profile" != "$profile" ]]; then
  echo "invite belongs to $returned_profile, not $profile" >&2
  exit 1
fi

config_dir=${XDG_CONFIG_HOME:-${HOME:?HOME is required}/.config}/wisp
config_file="$config_dir/friend.env"
mkdir -p "$config_dir"
temporary=$(mktemp "$config_dir/friend.env.XXXXXX")
trap 'rm -f -- "$temporary"' EXIT
chmod 0600 "$temporary"
printf 'WISP_FRIEND_HOST=%s\nWISP_SERVER_URL=%s\nWISP_PROFILE=%s\nWISP_DEVICE_ID=%s\nWISP_DEVICE_TOKEN=%s\nWISP_E2EE_KEY=%s\n' \
  "$host" "$server_url" "$profile" "$device_id" "$device_token" "$e2ee_key" >"$temporary"
mv -f -- "$temporary" "$config_file"
trap - EXIT

unset invite_code device_token e2ee_key
echo "Registered this device for $returned_profile through $host."
echo "Start Wisp with: just friend"
