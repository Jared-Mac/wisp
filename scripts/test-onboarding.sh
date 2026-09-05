#!/usr/bin/env bash
set -euo pipefail
repo_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
test_root=$(mktemp -d)
trap 'rm -rf -- "$test_root"' EXIT
mkdir -p "$test_root/bin" "$test_root/config" "$test_root/runtime"
chmod 0700 "$test_root/runtime"
cp "$repo_dir/scripts/wisp-launch.sh" "$test_root/bin/wisp-launch"
install -m 0755 "$repo_dir/tests/fixtures/account-launch-mock.sh" "$test_root/bin/wisp-client"
install -m 0755 "$repo_dir/tests/fixtures/account-launch-mock.sh" "$test_root/bin/wisp-onboarding"
for outcome in cancel success; do
  mkdir -p "$test_root/$outcome"
  XDG_RUNTIME_DIR="$test_root/runtime" WISP_TEST_STATE="$test_root/$outcome" \
    WISP_TEST_OUTCOME="$outcome" timeout 5 bash "$test_root/bin/wisp-launch" >/dev/null
  [[ $(rg -c '^prompt:login$' "$test_root/$outcome/calls") == 1 ]]
done
[[ $(rg -c '^client$' "$test_root/cancel/calls") == 1 ]]
[[ $(rg -c '^client$' "$test_root/success/calls") == 2 ]]

if command -v qs >/dev/null 2>&1; then
  cp -a "$repo_dir/quickshell/onboarding" "$test_root/onboarding"
  cp "$repo_dir/tests/quickshell/Onboarding.qml" "$test_root/shell.qml"
  for mode in "" login register; do
    result=0
    XDG_CONFIG_HOME="$test_root/config" XDG_RUNTIME_DIR="$test_root/runtime" \
      WISP_ONBOARDING_MODE="$mode" QT_QPA_PLATFORM=offscreen QT_QUICK_BACKEND=software \
      timeout 10 qs --path "$test_root" >"$test_root/qml.log" 2>&1 || result=$?
    [[ $result == 2 ]] || { cat "$test_root/qml.log"; exit 1; }
    rg -q 'ONBOARDING_OK' "$test_root/qml.log"
    if rg 'ONBOARDING_FAILED|ReferenceError|TypeError|Failed to load' "$test_root/qml.log"; then exit 1; fi
  done
fi
echo 'Login/register navigation and launch success/cancellation passed'
