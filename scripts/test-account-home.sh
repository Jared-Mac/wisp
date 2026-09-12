#!/usr/bin/env bash
set -euo pipefail
repo_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
test_dir=$(mktemp -d)
trap 'rm -rf -- "$test_dir"' EXIT
cp -a "$repo_dir/quickshell/app" "$test_dir/app"
cp "$repo_dir/tests/quickshell/AccountHome.qml" "$test_dir/shell.qml"
mkdir -p "$test_dir/app/native/WispVideo" "$test_dir/config"
cp "$repo_dir/target/video-ui/libwispvideo.so" "$repo_dir/target/video-ui/qmldir" "$test_dir/app/native/WispVideo/"
XDG_CONFIG_HOME="$test_dir/config" WISP_SOCKET="$test_dir/no-daemon.sock" \
  QML_IMPORT_PATH="$test_dir/app/native" QT_QPA_PLATFORM=offscreen QT_QUICK_BACKEND=software \
  timeout 15 qs --path "$test_dir" >"$test_dir/log" 2>&1 || { cat "$test_dir/log"; exit 1; }
if rg 'ACCOUNT_HOME_FAILED|TypeError|ReferenceError|Binding loop|Cannot assign|Failed to load' "$test_dir/log"; then cat "$test_dir/log"; exit 1; fi
rg -q ACCOUNT_HOME_OK "$test_dir/log"
echo 'Account Home, no-server permissions, friend search and membership transition passed'
