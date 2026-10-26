#!/usr/bin/env bash
set -euo pipefail
repo_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
test_dir=$(mktemp -d)
trap 'rm -rf -- "$test_dir"' EXIT
cp -a "$repo_dir/quickshell/app" "$test_dir/app"
cp "$repo_dir/tests/quickshell/Plugins.qml" "$test_dir/shell.qml"
for theme in ${WISP_TEST_THEMES:-soft_graphite daylight hearth clean_tui}; do
  mkdir -p "$test_dir/$theme"
  XDG_CONFIG_HOME="$test_dir/$theme" WISP_SOCKET="$test_dir/no-daemon.sock" WISP_TEST_THEME="$theme" \
    QT_QPA_PLATFORM=offscreen QT_QUICK_BACKEND=software timeout 20 qs --path "$test_dir" >"$test_dir/log" 2>&1 || { cat "$test_dir/log"; exit 1; }
  if rg 'PLUGINS_FAILED|TypeError|ReferenceError|Binding loop|Cannot assign|Unable to assign|Cannot anchor|Failed to load' "$test_dir/log"; then cat "$test_dir/log"; exit 1; fi
  rg -q PLUGINS_OK "$test_dir/log" || { cat "$test_dir/log"; exit 1; }
done
echo 'Plugin UI scope, draft, stale reply, and explicit action checks passed in all themes'
