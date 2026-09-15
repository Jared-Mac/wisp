#!/usr/bin/env bash
set -euo pipefail
repo_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
test_dir=$(mktemp -d)
trap 'rm -rf -- "$test_dir"' EXIT
mkdir -p "$test_dir/config"
cp -a "$repo_dir/quickshell/app" "$test_dir/app"
cp "$repo_dir/tests/quickshell/ServerModeration.qml" "$test_dir/shell.qml"
XDG_CONFIG_HOME="$test_dir/config" WISP_SOCKET="$test_dir/no-real-daemon.sock" \
  QT_QPA_PLATFORM=offscreen QT_QUICK_BACKEND=software \
  timeout 20 qs --path "$test_dir" >"$test_dir/log" 2>&1 || { cat "$test_dir/log"; exit 1; }
if rg 'SERVER_MODERATION_FAILED|TypeError|ReferenceError|Binding loop|Cannot assign|Cannot anchor|Failed to load' "$test_dir/log"; then cat "$test_dir/log"; exit 1; fi
rg -q SERVER_MODERATION_OK "$test_dir/log" || { cat "$test_dir/log"; exit 1; }
echo 'Offline moderation, confirmation, role restrictions, server scope, pending removal and compatibility passed'
