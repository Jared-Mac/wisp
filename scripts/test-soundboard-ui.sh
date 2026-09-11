#!/usr/bin/env bash
set -euo pipefail
repo_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
test_dir=$(mktemp -d)
trap 'rm -rf -- "$test_dir"' EXIT
mkdir -p "$test_dir/config"
cp -a "$repo_dir/quickshell/app" "$test_dir/app"
cp "$repo_dir/tests/quickshell/Soundboard.qml" "$test_dir/shell.qml"
platform=offscreen
panel_test=0
case "${1:-}" in
  --panel)
    [[ -n ${WAYLAND_DISPLAY:-} ]] || { echo 'The panel regression needs a Wayland session.' >&2; exit 2; }
    sed -i 's/FloatingWindow {/PanelWindow {/' "$test_dir/shell.qml"
    platform=wayland
    panel_test=1
    ;;
  "") ;;
  *) echo 'usage: test-soundboard-ui.sh [--panel]' >&2; exit 2 ;;
esac
XDG_CONFIG_HOME="$test_dir/config" WISP_TEST_PANEL="$panel_test" QT_QPA_PLATFORM="$platform" QT_QUICK_BACKEND=software \
  timeout 20 qs --path "$test_dir" >"$test_dir/log" 2>&1 || { cat "$test_dir/log"; exit 1; }
if rg 'SOUNDBOARD_FAILED|FAIL!|TypeError|ReferenceError|Binding loop|Cannot assign|Cannot anchor|Failed to load|Cannot specify' "$test_dir/log"; then cat "$test_dir/log"; exit 1; fi
rg -q SOUNDBOARD_OK "$test_dir/log" || { cat "$test_dir/log"; exit 1; }
echo 'Soundboard server isolation, permissions, mute, stale replies, Stop, upload drafts and starter-pack restoration passed'
if [[ $panel_test == 1 ]]; then echo 'Soundboard reopened successfully across three panel-window lifecycles'; fi
