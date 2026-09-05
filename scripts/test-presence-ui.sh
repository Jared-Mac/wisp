#!/usr/bin/env bash
set -euo pipefail
repo_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
test_dir=$(mktemp -d)
trap 'rm -rf -- "$test_dir"' EXIT
mkdir -p "$test_dir/config"
cp -a "$repo_dir/quickshell/app" "$test_dir/app"
cp "$repo_dir/tests/quickshell/Presence.qml" "$test_dir/shell.qml"
if [[ -f "$repo_dir/target/video-ui/libwispvideo.so" ]]; then
  mkdir -p "$test_dir/app/native/WispVideo"
  cp "$repo_dir/target/video-ui/libwispvideo.so" "$repo_dir/target/video-ui/qmldir" "$test_dir/app/native/WispVideo/"
fi
for presentation in app panel; do
  XDG_CONFIG_HOME="$test_dir/config" WISP_SOCKET="$test_dir/no-real-daemon.sock" \
    WISP_TEST_PRESENTATION="$presentation" QML_IMPORT_PATH="$test_dir/app/native" \
    QT_QPA_PLATFORM=offscreen QT_QUICK_BACKEND=software \
    timeout 25 qs --path "$test_dir" > "$test_dir/log" 2>&1 || { cat "$test_dir/log"; exit 1; }
  if rg 'PRESENCE_FAILED|TypeError|ReferenceError|Binding loop|Cannot assign|Cannot anchor|Failed to load' "$test_dir/log"; then cat "$test_dir/log"; exit 1; fi
  rg -q PRESENCE_OK "$test_dir/log" || { cat "$test_dir/log"; exit 1; }
done
echo 'Presence explanations and acknowledged knock feedback passed in app and panel'
