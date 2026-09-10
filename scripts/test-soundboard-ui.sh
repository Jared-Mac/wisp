#!/usr/bin/env bash
set -euo pipefail
repo_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
test_dir=$(mktemp -d)
trap 'rm -rf -- "$test_dir"' EXIT
mkdir -p "$test_dir/config"
cp -a "$repo_dir/quickshell/app" "$test_dir/app"
cp "$repo_dir/tests/quickshell/Soundboard.qml" "$test_dir/shell.qml"
XDG_CONFIG_HOME="$test_dir/config" QT_QPA_PLATFORM=offscreen QT_QUICK_BACKEND=software \
  timeout 20 qs --path "$test_dir" >"$test_dir/log" 2>&1 || { cat "$test_dir/log"; exit 1; }
if rg 'SOUNDBOARD_FAILED|FAIL!|TypeError|ReferenceError|Binding loop|Cannot assign|Cannot anchor|Failed to load|Cannot specify' "$test_dir/log"; then cat "$test_dir/log"; exit 1; fi
rg -q SOUNDBOARD_OK "$test_dir/log" || { cat "$test_dir/log"; exit 1; }
echo 'Soundboard server isolation, permissions, mute, stale replies, Stop, upload drafts and starter-pack restoration passed'
