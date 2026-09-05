#!/usr/bin/env bash
set -euo pipefail

repo_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
source_dir="$repo_dir/quickshell/"
plugin_root="${XDG_CONFIG_HOME:-$HOME/.config}/omarchy/plugins"
destination="$plugin_root/dev.wisp"
existing_only=false
case "${1:-}" in
  --existing) existing_only=true ;;
  "") ;;
  *) echo "usage: plugin-sync.sh [--existing]" >&2; exit 2 ;;
esac
if [[ "$existing_only" == true && ! -d "$destination" ]]; then exit 0; fi

omarchy plugin validate "$repo_dir/quickshell"
mkdir -p "$plugin_root"

if [[ -d "$destination" ]]; then
  backup_root="${XDG_STATE_HOME:-$HOME/.local/state}/wisp/backups"
  mkdir -p "$backup_root"
  backup_dir=$(mktemp -d "$backup_root/omarchy-adapter.XXXXXX")
  cp -a "$destination" "$backup_dir/dev.wisp"
  echo "Backed up previous Omarchy adapter to $backup_dir/dev.wisp"
fi

if command -v rsync >/dev/null 2>&1; then
  mkdir -p "$destination"
  rsync --archive --delete "$source_dir" "$destination/"
else
  mkdir -p "$destination"
  cp -a "$source_dir". "$destination/"
fi

# The shell registry does not know about a newly copied plugin until it has
# walked the user plugin directory. Discovery is asynchronous inside
# Quickshell, so wait briefly before asking Omarchy to enable it.
omarchy-shell shell rescanPlugins
for _ in $(seq 1 40); do
  if omarchy plugin list --json | jq -e 'any(.[]; .id == "dev.wisp")' >/dev/null; then
    break
  fi
  sleep 0.05
done

if ! omarchy plugin list --json | jq -e 'any(.[]; .id == "dev.wisp")' >/dev/null; then
  echo "Wisp was copied but Omarchy did not discover it at $destination" >&2
  exit 1
fi

if [[ "$existing_only" == false ]] && ! omarchy plugin list --json | jq -e 'any(.[]; .id == "dev.wisp" and .enabled == true)' >/dev/null; then
  omarchy plugin enable dev.wisp --section center
fi
echo "Synced Wisp to $destination"
