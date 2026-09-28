#!/usr/bin/env bash
set -euo pipefail
repo_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
test_dir=$(mktemp -d)
trap 'rm -rf -- "$test_dir"' EXIT
mkdir -p "$test_dir/config"
cp -a "${WISP_TEST_APP_SOURCE:-$repo_dir/quickshell/app}" "$test_dir/app"
cp "$repo_dir/tests/quickshell/TransferProgress.qml" "$test_dir/shell.qml"
XDG_CONFIG_HOME="$test_dir/config" WISP_SOCKET="$test_dir/no-daemon.sock" \
  QT_QPA_PLATFORM=offscreen QT_QUICK_BACKEND=software QT_QPA_PLATFORMTHEME=basic \
  timeout 20 qs --path "$test_dir" > "$test_dir/log" 2>&1 || { cat "$test_dir/log"; exit 1; }
if rg 'TRANSFER_PROGRESS_FAILED|TypeError|ReferenceError|Binding loop|Cannot assign|Failed to load' "$test_dir/log"; then
  cat "$test_dir/log"; exit 1
fi
rg -q TRANSFER_PROGRESS_OK "$test_dir/log" || { cat "$test_dir/log"; exit 1; }
echo 'QML transfer progress repeated-event cap, labels, disconnect and idle/completed expiry passed'
