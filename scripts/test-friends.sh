#!/usr/bin/env bash
set -euo pipefail
repo_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
test_dir=$(mktemp -d)
trap 'rm -rf -- "$test_dir"' EXIT
mkdir -p "$test_dir/config/wisp"
cp -a "$repo_dir/quickshell/app" "$test_dir/app"
cp "$repo_dir/tests/quickshell/Friends.qml" "$test_dir/shell.qml"
for reload in 0 1; do
  XDG_CONFIG_HOME="$test_dir/config" WISP_FRIENDS_RELOAD="$reload" \
    QT_QPA_PLATFORM=offscreen QT_QUICK_BACKEND=software timeout 10 qs --path "$test_dir" >"$test_dir/log" 2>&1
  if ! rg -q FRIENDS_OK "$test_dir/log" || rg -q 'FRIENDS_FAILED|Binding loop|TypeError|ReferenceError|Cannot assign|Failed to load' "$test_dir/log"; then
    cat "$test_dir/log"; exit 1
  fi
done
echo 'Favorite sorting, toggling, account isolation, and restart persistence passed'
