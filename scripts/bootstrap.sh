#!/usr/bin/env bash
set -euo pipefail

repo_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
cd "$repo_dir"

if ! command -v mise >/dev/null 2>&1; then
  echo "mise is required (it is included with Omarchy)." >&2
  exit 1
fi

mise trust "$repo_dir/mise.toml" >/dev/null 2>&1 || true
mise install
cargo fetch --locked 2>/dev/null || cargo fetch
./scripts/livekit-install.sh >/dev/null

mkdir -p "${XDG_CONFIG_HOME:-$HOME/.config}/wisp"
mkdir -p "${XDG_DATA_HOME:-$HOME/.local/share}/wisp/server"
mkdir -p "${XDG_STATE_HOME:-$HOME/.local/state}/wisp"
mkdir -p "${XDG_RUNTIME_DIR:?XDG_RUNTIME_DIR is required}/wisp"
./scripts/app-sync.sh >/dev/null

echo "Wisp development dependencies are ready."
echo "Run 'just dev', then 'just plugin-sync' in another terminal."
