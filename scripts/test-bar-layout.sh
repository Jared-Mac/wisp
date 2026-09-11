#!/usr/bin/env bash
set -euo pipefail
repo_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
test_dir=$(mktemp -d)
trap 'rm -rf -- "$test_dir"' EXIT
mkdir -p "$test_dir/config"
cp -a "$repo_dir/quickshell/app" "$test_dir/app"
cp "$repo_dir/tests/quickshell/BarLayout.qml" "$test_dir/shell.qml"
platform=offscreen
if [[ ${1:-} == --panel ]]; then
  [[ -n ${WAYLAND_DISPLAY:-} ]] || { echo 'Panel lifecycle checks require Wayland.' >&2; exit 2; }
  sed -i 's/FloatingWindow {/PanelWindow {/' "$test_dir/shell.qml"
  platform=wayland
elif [[ -n ${1:-} ]]; then
  echo 'usage: test-bar-layout.sh [--panel]' >&2; exit 2
fi
if [[ -f "$repo_dir/target/video-ui/libwispvideo.so" ]]; then
  mkdir -p "$test_dir/app/native/WispVideo"
  cp "$repo_dir/target/video-ui/libwispvideo.so" "$repo_dir/target/video-ui/qmldir" "$test_dir/app/native/WispVideo/"
fi
XDG_CONFIG_HOME="$test_dir/config" WISP_SOCKET="$test_dir/no-real-daemon.sock" \
  QML_IMPORT_PATH="$test_dir/app/native" QT_QPA_PLATFORM="$platform" QT_QUICK_BACKEND=software \
  timeout 25 qs --path "$test_dir" > "$test_dir/log" 2>&1 || { cat "$test_dir/log"; exit 1; }
if rg 'BAR_LAYOUT_FAILED|TypeError|ReferenceError|Binding loop|Cannot assign|Cannot anchor|Failed to load' "$test_dir/log"; then cat "$test_dir/log"; exit 1; fi
rg -q BAR_LAYOUT_OK "$test_dir/log" || { cat "$test_dir/log"; exit 1; }
echo 'Horizontal bar layout, narrow fallback, draft preservation and safe soundboard access passed'
