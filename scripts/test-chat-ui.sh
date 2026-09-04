#!/usr/bin/env bash
set -euo pipefail
repo_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
test_dir=$(mktemp -d)
trap 'rm -rf -- "$test_dir"' EXIT
mkdir -p "$test_dir/config/wisp"
cp -a "${WISP_TEST_APP_SOURCE:-$repo_dir/quickshell/app}" "$test_dir/app"
if [[ -f "$repo_dir/target/video-ui/libwispvideo.so" ]]; then
  mkdir -p "$test_dir/app/native/WispVideo"
  cp "$repo_dir/target/video-ui/libwispvideo.so" "$repo_dir/target/video-ui/qmldir" "$test_dir/app/native/WispVideo/"
fi
cp "$repo_dir/tests/quickshell/ChatWorkspace.qml" "$test_dir/shell.qml"
for fixture_mode in ${WISP_TEST_MODES:-app image panel settings edit files clearroom cleardm roomsettings newroom themes friends identity panelidentity identityactions panelidentityactions workspace saved traycollapse presence panelpresence picker tilemoves tiles tilesreload newchat addchat shortcuts audiotooltips panelaudiotooltips latest panellatest imagegeometry panelimagegeometry roomcleanup cleantui returnchat invites panelinvites}; do
  screenshot=""
  if [[ -n "${WISP_CHAT_SCREENSHOT:-}" ]]; then
    screenshot="${WISP_CHAT_SCREENSHOT%.png}-$fixture_mode.png"
  fi
  XDG_CONFIG_HOME="$test_dir/config" WISP_SOCKET="$test_dir/wisp/wispd.sock" \
    WISP_CHAT_FIXTURE_MODE="$fixture_mode" WISP_CHAT_SCREENSHOT="$screenshot" \
    QML_IMPORT_PATH="$test_dir/app/native" QT_QPA_PLATFORM=offscreen QT_QUICK_BACKEND=software \
    timeout 20 qs --path "$test_dir" >"$test_dir/log" 2>&1 || { cat "$test_dir/log"; exit 1; }
  if rg 'CHAT_TEST_FAILED|TypeError|ReferenceError|Binding loop|Cannot assign|Cannot anchor|Failed to load' "$test_dir/log"; then
    cat "$test_dir/log"
    exit 1
  fi
  rg -q 'CHAT_WORKSPACE_OK' "$test_dir/log"
done
jq -e '.muted == true and .volume == 35 and .soundPath == "file:///tmp/test-custom-sound.wav"' "$test_dir/config/wisp/notifications.json" >/dev/null || { cat "$test_dir/config/wisp/notifications.json"; exit 1; }
echo 'App/tray attachments, room permissions, clear confirmation, drafts, and notification settings passed'
