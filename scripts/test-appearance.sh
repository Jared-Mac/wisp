#!/usr/bin/env bash
set -euo pipefail
repo_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
test_dir=$(mktemp -d)
trap 'rm -rf -- "$test_dir"' EXIT
mkdir -p "$test_dir/config/wisp"
cp -a "$repo_dir/quickshell/app" "$test_dir/app"
cp "$repo_dir/tests/quickshell/Appearance.qml" "$test_dir/shell.qml"
for environment in cachyos omarchy unknown; do
  for requested in terminal-experimental legacy invalid missing; do
    if [[ $requested == missing ]]; then
      rm -f -- "$test_dir/config/wisp/appearance.json"
    else
      jq -cn --arg profile "$requested" '{profile:$profile}' >"$test_dir/config/wisp/appearance.json"
    fi
    expected=legacy
    if [[ $environment == cachyos && $requested == terminal-experimental ]]; then expected=terminal-experimental; fi
    XDG_CONFIG_HOME="$test_dir/config" WISP_APPEARANCE_ENVIRONMENT="$environment" WISP_EXPECT_APPEARANCE="$expected" \
      QT_QPA_PLATFORM=offscreen QT_QUICK_BACKEND=software timeout 10 qs --path "$test_dir" >"$test_dir/log" 2>&1
    if ! rg -q APPEARANCE_OK "$test_dir/log" || rg -q 'APPEARANCE_FAILED|Binding loop|TypeError|ReferenceError|Cannot assign|Failed to load' "$test_dir/log"; then
      cat "$test_dir/log"; exit 1
    fi
  done
done
echo 'Appearance selection: CachyOS trial, legacy/disabled, Omarchy, and unknown environments passed'
