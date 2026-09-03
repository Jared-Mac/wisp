#!/usr/bin/env bash
set -euo pipefail

release_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
bin_root=${XDG_BIN_HOME:-${HOME:?HOME is required}/.local/bin}

mkdir -p "$bin_root"

for binary in wispd wispctl wisp-server; do
  if [[ ! -x "$release_dir/bin/$binary" ]]; then
    echo "release archive is missing bin/$binary" >&2
    exit 1
  fi
done
install -m 0755 "$release_dir/scripts/backup-database.sh" "$bin_root/wisp-backup"
install -m 0755 "$release_dir/scripts/restore-database.sh" "$bin_root/wisp-restore"
for binary in wispd wispctl wisp-server; do
  install -m 0755 "$release_dir/bin/$binary" "$bin_root/$binary"
done

"$release_dir/scripts/app-sync.sh"

echo "Installed Wisp binaries to $bin_root"
echo "Enroll a client with: wisp-friend-register <tailscale-host> <Tyler|Jack|Charlie>"
echo "Then start Wisp from the application menu or run: wisp"
