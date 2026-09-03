#!/usr/bin/env bash
set -euo pipefail

host=${1:-}
profile=${2:-}

if [[ -z "$host" || -z "$profile" ]]; then
  echo "usage: just friend-register <tailscale-host-or-ip> <Tyler|Jack|Charlie>" >&2
  exit 2
fi
if [[ ! "$host" =~ ^[A-Za-z0-9.-]+$ ]]; then
  echo "host must be a Tailscale DNS name or IP address" >&2
  exit 2
fi
case "$profile" in
  Tyler|Jack|Charlie) ;;
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
  read -rsp "Private media key: " e2ee_key
  echo
fi
if (( ${#e2ee_key} < 16 )); then
  echo "private media key must contain at least 16 characters" >&2
  exit 2
fi
if [[ ! "$e2ee_key" =~ ^[A-Za-z0-9_-]{16,128}$ ]]; then
  echo "private media key must use 16–128 letters, digits, '_' or '-'" >&2
  exit 2
fi

device_name=${HOSTNAME:-CachyOS device}
response=$(jq -n \
  --arg invite "$invite_code" \
  --arg device "$device_name" \
  '{invite_code:$invite, device_name:$device, protocol_version:1}' \
  | curl --silent --show-error --fail-with-body \
      -H 'content-type: application/json' \
      --data-binary @- \
      "http://$host:8787/v1/devices/register")

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
printf 'WISP_FRIEND_HOST=%s\nWISP_PROFILE=%s\nWISP_DEVICE_ID=%s\nWISP_DEVICE_TOKEN=%s\nWISP_E2EE_KEY=%s\n' \
  "$host" "$profile" "$device_id" "$device_token" "$e2ee_key" >"$temporary"
mv -f -- "$temporary" "$config_file"
trap - EXIT

unset invite_code device_token e2ee_key
echo "Registered this device for $returned_profile through $host."
echo "Start Wisp with: just friend"
