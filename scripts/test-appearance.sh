#!/usr/bin/env bash
set -euo pipefail
repo_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
test_dir=$(mktemp -d)
trap 'rm -rf -- "$test_dir"' EXIT
mkdir -p "$test_dir/config/wisp"
cp -a "$repo_dir/quickshell/app" "$test_dir/app"
cp "$repo_dir/tests/quickshell/Appearance.qml" "$test_dir/shell.qml"
for environment in cachyos desktop omarchy unknown; do
  for requested in terminal terminal-experimental classic legacy invalid missing; do
    if [[ $requested == missing ]]; then
      rm -f -- "$test_dir/config/wisp/appearance.json"
    else
      jq -cn --arg profile "$requested" '{profile:$profile}' >"$test_dir/config/wisp/appearance.json"
    fi
    expected=legacy
    if [[ $environment != omarchy && $requested != legacy && $requested != classic ]]; then
      if [[ $environment != unknown || $requested == terminal || $requested == terminal-experimental ]]; then expected=terminal; fi
    fi
    XDG_CONFIG_HOME="$test_dir/config" WISP_APPEARANCE_ENVIRONMENT="$environment" WISP_EXPECT_APPEARANCE="$expected" \
      QT_QPA_PLATFORM=offscreen QT_QUICK_BACKEND=software timeout 10 qs --path "$test_dir" >"$test_dir/log" 2>&1
    if ! rg -q APPEARANCE_OK "$test_dir/log" || rg -q 'APPEARANCE_FAILED|Binding loop|TypeError|ReferenceError|Cannot assign|Failed to load' "$test_dir/log"; then
      cat "$test_dir/log"; exit 1
    fi
  done
done
for change in legacy terminal; do
  current=$(jq -r '.profile' "$test_dir/config/wisp/appearance.json" 2>/dev/null || echo missing)
  expected=terminal
  [[ $current != legacy ]] || expected=legacy
  XDG_CONFIG_HOME="$test_dir/config" WISP_APPEARANCE_ENVIRONMENT=desktop WISP_EXPECT_APPEARANCE="$expected" \
    WISP_TEST_CHANGE="$change" WISP_EXPECT_CHANGED="$change" QT_QPA_PLATFORM=offscreen QT_QUICK_BACKEND=software \
    timeout 10 qs --path "$test_dir" >"$test_dir/log" 2>&1
  if ! rg -q APPEARANCE_SWITCH_OK "$test_dir/log" || rg -q 'APPEARANCE_FAILED|Binding loop|TypeError|ReferenceError|Cannot assign|Failed to load' "$test_dir/log"; then cat "$test_dir/log"; exit 1; fi
  jq -e --arg profile "$change" '.profile == $profile' "$test_dir/config/wisp/appearance.json" >/dev/null
done
for palette in graphite violet ember wisp; do
  for reload in 0 1; do
    XDG_CONFIG_HOME="$test_dir/config" WISP_APPEARANCE_ENVIRONMENT=desktop \
      WISP_PALETTE_PERSIST="$palette" WISP_PALETTE_RELOAD="$reload" QT_QPA_PLATFORM=offscreen QT_QUICK_BACKEND=software \
      timeout 10 qs --path "$test_dir" >"$test_dir/log" 2>&1
    if ! rg -q PALETTE_PERSIST_OK "$test_dir/log" || rg -q 'APPEARANCE_FAILED|Binding loop|TypeError|ReferenceError|Cannot assign|Failed to load' "$test_dir/log"; then cat "$test_dir/log"; exit 1; fi
    jq -e --arg palette "$palette" '.palette == $palette' "$test_dir/config/wisp/appearance.json" >/dev/null
  done
done
echo 'Theme defaults, palette switching/persistence, backwards compatibility, Omarchy guards, and live switching passed'
