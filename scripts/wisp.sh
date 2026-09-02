#!/usr/bin/env bash
set -euo pipefail

bin_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
config_file=${XDG_CONFIG_HOME:-${HOME:?HOME is required}/.config}/wisp/friend.env
if [[ -f "$config_file" ]]; then
  configured_profile=$(sed -n 's/^WISP_PROFILE=//p' "$config_file" | tail -n 1)
  case "$configured_profile" in
    Tyler|Jack|Charlie) export WISP_PROFILE="$configured_profile" ;;
  esac
fi

if command -v systemctl >/dev/null 2>&1 \
  && systemctl --user show-environment >/dev/null 2>&1; then
  systemctl --user start wisp.service
else
  nohup "$bin_dir/wisp-launch" </dev/null >/dev/null 2>&1 &
fi

exec "$bin_dir/wisp-ui" open >/dev/null 2>&1
