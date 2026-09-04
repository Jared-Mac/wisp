#!/usr/bin/env bash
set -euo pipefail
repo_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
"${NODE:-node}" "$repo_dir/scripts/test-chat-colors.cjs"
test_dir=$(mktemp -d)
trap 'rm -rf -- "$test_dir"' EXIT
cp -a "$repo_dir/quickshell/app" "$test_dir/app"
cp "$repo_dir/tests/quickshell/ChatColors.qml" "$test_dir/shell.qml"
for pass in save reload; do
  XDG_CONFIG_HOME="$test_dir/config" WISP_SOCKET="$test_dir/offline.sock" \
    QT_QPA_PLATFORM=offscreen QT_QUICK_BACKEND=software \
    timeout 10 qs --path "$test_dir" >"$test_dir/log" 2>&1 || { cat "$test_dir/log"; exit 1; }
  if ! rg -q CHAT_COLORS_OK "$test_dir/log" || rg -q 'CHAT_COLORS_FAILED|TypeError|ReferenceError|Binding loop|Cannot assign|Failed to load' "$test_dir/log"; then
    cat "$test_dir/log"; exit 1
  fi
  jq -e '.assignments | fromjson | .charlie == "#7fa9cf" and .jared == "#cb8897"' "$test_dir/config/wisp/chat-colors.json" >/dev/null
done
echo 'Chat border colors persist across restart and follow conversation identity across views'
