#!/usr/bin/env bash
set -euo pipefail

release_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
bin_root=${XDG_BIN_HOME:-${HOME:?HOME is required}/.local/bin}
state_root=${XDG_STATE_HOME:-${HOME:?HOME is required}/.local/state}
config_root=${XDG_CONFIG_HOME:-${HOME:?HOME is required}/.config}

mkdir -p "$bin_root"

for binary in wispd wisp-account wispctl wisp-server; do
  if [[ ! -x "$release_dir/bin/$binary" ]]; then
    echo "release archive is missing bin/$binary" >&2
    exit 1
  fi
done

backup_dir=
for binary in wispd wisp-account wispctl wisp-server; do
  if [[ -e "$bin_root/$binary" ]]; then
    backup_dir="$state_root/wisp/backups/$(date --utc +%Y%m%dT%H%M%SZ)/bin"
    mkdir -p "$backup_dir"
    for installed_binary in wispd wisp-account wispctl wisp-server; do
      if [[ -e "$bin_root/$installed_binary" ]]; then
        install -m 0755 "$bin_root/$installed_binary" "$backup_dir/$installed_binary"
      fi
    done
    break
  fi
done

install -m 0755 "$release_dir/scripts/backup-database.sh" "$bin_root/wisp-backup"
install -m 0755 "$release_dir/scripts/restore-database.sh" "$bin_root/wisp-restore"
install -m 0755 "$release_dir/scripts/wisp-update.sh" "$bin_root/wisp-update"
if [[ -f "$release_dir/update-repository" ]]; then
  mkdir -p "$config_root/wisp"
  install -m 0644 "$release_dir/update-repository" "$config_root/wisp/update-repository"
fi
for binary in wispd wisp-account wispctl wisp-server; do
  install -m 0755 "$release_dir/bin/$binary" "$bin_root/$binary"
done

"$release_dir/scripts/app-sync.sh"

# Update an adapter the user already installed, but never install or enable
# Omarchy integration on their behalf.
if [[ -d "$config_root/omarchy/plugins/dev.wisp" ]]; then
  "$release_dir/scripts/plugin-sync.sh" --existing
fi

echo "Installed Wisp binaries to $bin_root"
if [[ -n "$backup_dir" ]]; then
  echo "Backed up previous Wisp binaries to $backup_dir"
fi
echo "Start Wisp from the application menu or run: wisp"
