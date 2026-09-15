#!/usr/bin/env bash
set -euo pipefail
repo_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
test_dir=$(mktemp -d)
trap 'rm -rf -- "$test_dir"' EXIT
cp -a "$repo_dir/quickshell/app" "$test_dir/app"
cp "$repo_dir/tests/quickshell/Typing.qml" "$test_dir/shell.qml"
QT_QPA_PLATFORM=offscreen QT_QUICK_BACKEND=software timeout 15 qs --path "$test_dir" >"$test_dir/log" 2>&1 || { cat "$test_dir/log";exit 1; }
if rg 'TYPING_FAILED|TypeError|ReferenceError|Binding loop|Cannot assign|Failed to load' "$test_dir/log";then cat "$test_dir/log";exit 1;fi
rg -q TYPING_OK "$test_dir/log" || { cat "$test_dir/log";exit 1; }
echo 'Typing activity, throttling, stop, inactivity expiry, permissions and server isolation passed'
