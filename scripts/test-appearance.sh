#!/usr/bin/env bash
set -euo pipefail
repo_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
test_dir=$(mktemp -d)
trap 'rm -rf -- "$test_dir"' EXIT
cp -a "$repo_dir/quickshell/app" "$test_dir/app"
cp "$repo_dir/tests/quickshell/Appearance.qml" "$test_dir/shell.qml"
run_case() {
  local name=$1 json=$2 profile=$3 palette=$4 environment=${5:-desktop} managed=${6:-0}
  mkdir -p "$test_dir/$name/wisp"
  jq -cn --argjson value "$json" '$value' >"$test_dir/$name/wisp/appearance.json"
  XDG_CONFIG_HOME="$test_dir/$name" WISP_APPEARANCE_ENVIRONMENT="$environment" WISP_TEST_MANAGED="$managed" \
    WISP_EXPECT_APPEARANCE="$profile" WISP_EXPECT_PALETTE="$palette" QT_QPA_PLATFORM=offscreen QT_QUICK_BACKEND=software \
    timeout 10 qs --path "$test_dir" >"$test_dir/log" 2>&1
  if ! rg -q APPEARANCE_OK "$test_dir/log" || rg -q 'APPEARANCE_FAILED|Binding loop|TypeError|ReferenceError|Cannot assign|Failed to load' "$test_dir/log"; then cat "$test_dir/log"; exit 1; fi
}
run_case default '{}' performative ash_olive
run_case old-performative '{"profile":"terminal","palette":"performative"}' performative ash_olive
run_case old-classic-performative '{"profile":"legacy","palette":"performative"}' performative ash_olive
run_case old-herdr '{"profile":"terminal","palette":"herdr"}' herdr herdr
run_case old-clean '{"profile":"clean_tui","palette":"performative"}' clean_tui ash_olive
run_case old-clean-herdr '{"profile":"clean-tui","palette":"herdr"}' clean_tui herdr
run_case old-terminal '{"profile":"terminal-experimental"}' terminal wisp
run_case old-classic '{"profile":"classic"}' legacy wisp
run_case unknown '{}' legacy wisp unknown
run_case managed '{"profile":"performative","palette":"performative","colorOptions":{"roomSections":false}}' legacy wisp omarchy 1
run_case omarchy-standalone '{"profile":"clean_tui","palette":"herdr"}' clean_tui herdr omarchy
run_case new-classic '{"version":2,"profile":"legacy","palette":"ash_olive"}' legacy ash_olive
run_case new-performative '{"version":2,"profile":"performative","palette":"violet"}' performative violet
XDG_CONFIG_HOME="$test_dir/default" WISP_APPEARANCE_ENVIRONMENT=desktop WISP_EXPECT_APPEARANCE=clean_tui WISP_EXPECT_PALETTE=ash_olive \
  WISP_TEST_RELOAD=1 QT_QPA_PLATFORM=offscreen QT_QUICK_BACKEND=software timeout 10 qs --path "$test_dir" >"$test_dir/log" 2>&1
if ! rg -q APPEARANCE_OK "$test_dir/log" || rg -q 'APPEARANCE_FAILED|Binding loop|TypeError|ReferenceError' "$test_dir/log"; then cat "$test_dir/log"; exit 1; fi
jq -e '.version == 2 and .profile == "clean_tui" and .palette == "ash_olive" and .colorOptions.chatBorders and (.colorOptions.roomSections | not)' "$test_dir/default/wisp/appearance.json" >/dev/null
echo 'Appearance/palette combinations, six independent color controls, migration, persistence and host ownership passed'
