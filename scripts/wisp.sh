#!/usr/bin/env bash
set -euo pipefail

bin_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
config_file=${XDG_CONFIG_HOME:-${HOME:?HOME is required}/.config}/wisp/account.env
if [[ -f "$config_file" ]]; then
  configured_profile=$(sed -n 's/^WISP_PROFILE=//p' "$config_file" | tail -n 1)
  if [[ -n "$configured_profile" && "$configured_profile" != *$'\n'* && "$configured_profile" != *$'\r'* ]]; then
    export WISP_PROFILE="$configured_profile"
  fi
fi

if command -v systemctl >/dev/null 2>&1 \
  && systemctl --user show-environment >/dev/null 2>&1; then
  already_running=false
  if systemctl --user is-active --quiet wisp.service; then
    already_running=true
  fi
  systemctl --user start wisp.service
  if [[ "$already_running" == true ]]; then
    exec "$bin_dir/wisp-ui" app open >/dev/null 2>&1
  fi
else
  if pgrep -u "$(id -u)" -x wispd >/dev/null 2>&1; then
    exec "$bin_dir/wisp-ui" app open >/dev/null 2>&1
  fi
  nohup "$bin_dir/wisp-launch" </dev/null >/dev/null 2>&1 &
fi
