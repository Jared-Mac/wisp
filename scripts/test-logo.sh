#!/usr/bin/env bash
set -euo pipefail

repo_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
test_dir=$(mktemp -d)
trap 'rm -rf -- "$test_dir"' EXIT

[[ -f "$repo_dir/quickshell/app/assets/wisp-icon.svg" ]]
[[ -f "$repo_dir/quickshell/app/assets/wisp-icon-tray.png" ]]
rg -q 'wisp-icon\.svg.*dev\.wisp\.svg' "$repo_dir/scripts/app-sync.sh"

cp -a "$repo_dir/quickshell/app" "$test_dir/app"
cp "$repo_dir/tests/quickshell/Logo.qml" "$test_dir/shell.qml"

QT_QPA_PLATFORM=offscreen QT_QUICK_BACKEND=software \
  timeout 10 qs --path "$test_dir" >"$test_dir/log" 2>&1

if ! rg -q LOGO_OK "$test_dir/log" \
  || rg -q 'LOGO_FAILED|Binding loop|TypeError|ReferenceError|Cannot assign|Failed to load' "$test_dir/log"; then
  cat "$test_dir/log"
  exit 1
fi

echo "Themeable QML wordmark, horizontal waveform and palette updates passed"
