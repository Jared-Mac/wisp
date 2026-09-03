#!/usr/bin/env bash
set -euo pipefail

release_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
bin_root=${XDG_BIN_HOME:-${HOME:?HOME is required}/.local/bin}

for binary in wispd wispctl wisp-server; do
  if [[ ! -x "$release_dir/bin/$binary" ]]; then
    echo "release archive is missing bin/$binary" >&2
    exit 1
  fi
done

mkdir -p "$bin_root"
for binary in wispd wispctl wisp-server; do
  install -m 0755 "$release_dir/bin/$binary" "$bin_root/$binary"
done

"$release_dir/scripts/app-sync.sh"

echo "Installed Wisp binaries to $bin_root"
echo "Configure a client with: wisp-friend-config <tailscale-host> <Tyler|Jack|Charlie>"
echo "Then start Wisp from the application menu or run: wisp"
