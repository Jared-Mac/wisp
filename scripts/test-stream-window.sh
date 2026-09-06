#!/usr/bin/env bash
set -euo pipefail
repo_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
test_dir=$(mktemp -d)
trap 'rm -rf -- "$test_dir"' EXIT
bash "$repo_dir/scripts/build-video-ui.sh"
cp -a "$repo_dir/quickshell/app" "$test_dir/app"
mkdir -p "$test_dir/app/native/WispVideo"
cp "$repo_dir/target/video-ui/"{libwispvideo.so,qmldir} "$test_dir/app/native/WispVideo/"
cp "$repo_dir/tests/quickshell/StreamWindow.qml" "$test_dir/shell.qml"
QML_IMPORT_PATH="$test_dir/app/native" XDG_CONFIG_HOME="$test_dir/config" WISP_SOCKET="$test_dir/test.sock" QT_QPA_PLATFORM=offscreen QT_QUICK_BACKEND=software \
  timeout 10 qs --path "$test_dir" >"$test_dir/log" 2>&1 || { cat "$test_dir/log"; exit 1; }
cat "$test_dir/log"
! rg 'STREAM_WINDOW_FAILED|TypeError|ReferenceError|Binding loop|Cannot assign|Failed to load' "$test_dir/log"
rg -q STREAM_WINDOW_OK "$test_dir/log"
