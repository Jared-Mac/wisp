#!/usr/bin/env bash
set -euo pipefail
repo_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
test_dir=$(mktemp -d)
trap 'rm -rf -- "$test_dir"' EXIT
cp -a "$repo_dir/quickshell/app" "$test_dir/app"
cp "$repo_dir/tests/quickshell/RoomMedia.qml" "$test_dir/shell.qml"
XDG_CONFIG_HOME="$test_dir/config" WISP_SOCKET="$test_dir/offline.sock" \
  QT_QPA_PLATFORM=offscreen QT_QUICK_BACKEND=software \
  timeout 10 qs --path "$test_dir" >"$test_dir/log" 2>&1
cat "$test_dir/log"
if ! rg -q ROOM_MEDIA_OK "$test_dir/log" || rg -q 'ROOM_MEDIA_FAILED|TypeError|ReferenceError|Binding loop|Cannot assign|Failed to load' "$test_dir/log"; then exit 1; fi
echo 'Wrapping room members and highlighted stop actions passed across appearances'
