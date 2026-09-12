#!/usr/bin/env bash
set -euo pipefail
repo_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
test_dir=$(mktemp -d)
trap 'rm -rf -- "$test_dir"' EXIT
cp -a "$repo_dir/quickshell/app" "$test_dir/app"
cp "$repo_dir/tests/quickshell/AccountRecovery.qml" "$test_dir/shell.qml"
mkdir -p "$test_dir/app/native/WispVideo"
video_ui="${WISP_TEST_VIDEO_UI_DIR:-${CARGO_TARGET_DIR:-$repo_dir/target}/video-ui}"
cp "$video_ui/libwispvideo.so" "$video_ui/qmldir" "$test_dir/app/native/WispVideo/"
for theme in ${WISP_TEST_THEMES:-soft_graphite daylight hearth clean_tui}; do
  mkdir -p "$test_dir/$theme/wisp"
  XDG_CONFIG_HOME="$test_dir/$theme" WISP_SOCKET="$test_dir/no-daemon.sock" WISP_TEST_THEME="$theme" \
    QML_IMPORT_PATH="$test_dir/app/native" QT_QPA_PLATFORM=offscreen QT_QUICK_BACKEND=software \
    timeout 20 qs --path "$test_dir" >"$test_dir/log" 2>&1 || { cat "$test_dir/log"; exit 1; }
  if rg 'ACCOUNT_RECOVERY_FAILED|TypeError|ReferenceError|Binding loop|Cannot assign|Cannot anchor|Failed to load' "$test_dir/log"; then cat "$test_dir/log"; exit 1; fi
  rg -q ACCOUNT_RECOVERY_OK "$test_dir/log" || { cat "$test_dir/log"; exit 1; }
done
echo 'Recovery email settings and dismissible errors passed in all themes'
