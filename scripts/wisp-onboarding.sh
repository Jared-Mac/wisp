#!/usr/bin/env bash
set -euo pipefail

if ! command -v qs >/dev/null 2>&1; then
  echo "Quickshell is required to run Wisp onboarding" >&2
  exit 1
fi

if [[ -n ${WISP_ONBOARDING_PATH:-} ]]; then
  exec qs --path "$WISP_ONBOARDING_PATH"
fi

installed=${XDG_CONFIG_HOME:-${HOME:?HOME is required}/.config}/quickshell/wisp-onboarding
if [[ -f "$installed/shell.qml" ]]; then
  exec qs --config wisp-onboarding
fi

repo_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
exec qs --path "$repo_dir/quickshell/onboarding"
