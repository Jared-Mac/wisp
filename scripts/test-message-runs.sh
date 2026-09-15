#!/usr/bin/env bash
set -euo pipefail
repo_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
test_dir=$(mktemp -d)
trap 'rm -rf -- "$test_dir"' EXIT
cp -a "$repo_dir/quickshell/app" "$test_dir/app"
cp "$repo_dir/tests/quickshell/MessageRuns.qml" "$test_dir/shell.qml"
mkdir -p "$test_dir/app/native/WispVideo"
cp "$repo_dir/target/video-ui/libwispvideo.so" "$repo_dir/target/video-ui/qmldir" "$test_dir/app/native/WispVideo/"
for layout in ${WISP_TEST_CHAT_LAYOUTS:-grouped compact soft_groups}; do
for theme in ${WISP_TEST_THEMES:-soft_graphite daylight hearth clean_tui}; do
  mkdir -p "$test_dir/$theme/wisp"
  XDG_CONFIG_HOME="$test_dir/$theme" WISP_SOCKET="$test_dir/no-daemon.sock" WISP_TEST_THEME="$theme" WISP_TEST_CHAT_LAYOUT="$layout" \
    QML_IMPORT_PATH="$test_dir/app/native" QT_QPA_PLATFORM=offscreen QT_QUICK_BACKEND=software \
    timeout 20 qs --path "$test_dir" >"$test_dir/log" 2>&1 || { cat "$test_dir/log"; exit 1; }
  if rg 'MESSAGE_RUNS_FAILED|TypeError|ReferenceError|Binding loop|Cannot assign|Cannot anchor|Failed to load' "$test_dir/log"; then cat "$test_dir/log"; exit 1; fi
  rg -q MESSAGE_RUNS_OK "$test_dir/log" || { cat "$test_dir/log"; exit 1; }
done
done
echo 'Three chat layouts, grouped avatars, hover/focus actions, stable spacing and invite restrictions passed'
