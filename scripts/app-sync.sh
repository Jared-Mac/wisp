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
desktop_template="$repo_dir/infra/local/dev.wisp.desktop"
service_root="${XDG_CONFIG_HOME:-$HOME/.config}/systemd/user"
service_template="$repo_dir/infra/local/dev.wisp.service"

if [[ ! -f "$source_dir/shell.qml" ]]; then
  echo "Standalone Wisp shell is missing from $source_dir" >&2
  exit 1
fi

mkdir -p "$destination" "$bin_root" "$desktop_root" "$icon_root" "$service_root"

if [[ -d "$repo_dir/native/video" ]]; then
  bash "$repo_dir/scripts/build-video-ui.sh"
fi

if command -v rsync >/dev/null 2>&1; then
  rsync --archive --delete "$source_dir" "$destination/"
else
  cp -a "$source_dir". "$destination/"
fi

if [[ -f "$repo_dir/target/video-ui/libwispvideo.so" ]]; then
  mkdir -p "$destination/native/WispVideo"
  install -m 0755 "$repo_dir/target/video-ui/libwispvideo.so" "$destination/native/WispVideo/"
  install -m 0644 "$repo_dir/target/video-ui/qmldir" "$destination/native/WispVideo/"
fi

install -m 0755 "$repo_dir/scripts/wisp-ui.sh" "$bin_root/wisp-ui"
install -m 0644 "$repo_dir/scripts/server-endpoint.sh" "$bin_root/wisp-server-endpoint"
install -m 0755 "$repo_dir/scripts/configure-friend.sh" "$bin_root/wisp-friend-config"
install -m 0755 "$repo_dir/scripts/register-friend-device.sh" "$bin_root/wisp-friend-register"
install -m 0755 "$repo_dir/scripts/friend-tailscale.sh" "$bin_root/wisp-friend"
install -m 0755 "$repo_dir/scripts/wisp-update.sh" "$bin_root/wisp-update"
install -m 0755 "$repo_dir/scripts/wisp-launch.sh" "$bin_root/wisp-launch"
install -m 0755 "$repo_dir/scripts/wisp.sh" "$bin_root/wisp"
desktop_contents=$(<"$desktop_template")
app_exec="$bin_root/wisp"
desktop_contents=${desktop_contents//@WISP_LAUNCH_EXEC@/$app_exec}
desktop_file=$(mktemp "$desktop_root/dev.wisp.desktop.XXXXXX")
trap 'rm -f -- "$desktop_file"' EXIT
printf '%s\n' "$desktop_contents" >"$desktop_file"
install -m 0644 "$desktop_file" "$desktop_root/dev.wisp.desktop"
rm -f -- "$desktop_file"
trap - EXIT
service_contents=$(<"$service_template")
launch_exec="$bin_root/wisp-launch"
service_contents=${service_contents//@WISP_LAUNCH_EXEC@/$launch_exec}
service_file=$(mktemp "$service_root/wisp.service.XXXXXX")
trap 'rm -f -- "$service_file"' EXIT
printf '%s\n' "$service_contents" >"$service_file"
install -m 0644 "$service_file" "$service_root/wisp.service"
rm -f -- "$service_file"
trap - EXIT
install -m 0644 "$source_dir/assets/waveform.svg" "$icon_root/dev.wisp.svg"

if command -v systemctl >/dev/null 2>&1; then
  systemctl --user daemon-reload
fi

if command -v kbuildsycoca6 >/dev/null 2>&1; then
  kbuildsycoca6 >/dev/null 2>&1 || true
elif command -v update-desktop-database >/dev/null 2>&1; then
  update-desktop-database "$desktop_root" >/dev/null 2>&1 || true
fi

echo "Synced standalone Wisp UI to $destination"
echo "Installed application launcher at $bin_root/wisp"
echo "Installed UI control helper at $bin_root/wisp-ui"
echo "Installed prebuilt release updater at $bin_root/wisp-update"
