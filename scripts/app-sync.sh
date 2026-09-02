#!/usr/bin/env bash
set -euo pipefail

repo_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
source_dir="$repo_dir/quickshell/app/"
config_root="${XDG_CONFIG_HOME:-$HOME/.config}/quickshell"
destination="$config_root/wisp"
bin_root="${XDG_BIN_HOME:-$HOME/.local/bin}"
data_root="${XDG_DATA_HOME:-$HOME/.local/share}"
desktop_root="$data_root/applications"
icon_root="$data_root/icons/hicolor/scalable/apps"

if [[ ! -f "$source_dir/shell.qml" ]]; then
  echo "Standalone Wisp shell is missing from $source_dir" >&2
  exit 1
fi

mkdir -p "$destination" "$bin_root" "$desktop_root" "$icon_root"

if command -v rsync >/dev/null 2>&1; then
  rsync --archive --delete "$source_dir" "$destination/"
else
  cp -a "$source_dir". "$destination/"
fi

install -m 0755 "$repo_dir/scripts/wisp-ui.sh" "$bin_root/wisp-ui"
install -m 0644 "$repo_dir/infra/local/dev.wisp.desktop" "$desktop_root/dev.wisp.desktop"
install -m 0644 "$source_dir/assets/waveform.svg" "$icon_root/dev.wisp.svg"

echo "Synced standalone Wisp UI to $destination"
echo "Installed launcher at $bin_root/wisp-ui"
