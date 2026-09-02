#!/usr/bin/env bash
set -euo pipefail

repo_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
"$repo_dir/scripts/plugin-sync.sh"

if ! command -v inotifywait >/dev/null 2>&1; then
  echo "inotifywait is required for plugin-watch; run 'omarchy pkg add inotify-tools'." >&2
  exit 1
fi

while inotifywait -qq -r -e close_write,create,delete,move "$repo_dir/quickshell"; do
  "$repo_dir/scripts/plugin-sync.sh"
done
