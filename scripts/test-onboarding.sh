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

# Exercise the entry point used by both Omarchy actions. Never contact the
# user's systemd instance, daemon, or saved account configuration in this test.
cp "$repo_dir/scripts/wisp.sh" "$test_root/bin/wisp"
chmod +x "$test_root/bin/wisp" "$test_root/bin/wisp-launch"
for mock in systemctl pgrep wisp-ui; do
  install -m 0755 "$repo_dir/tests/fixtures/session-launch-mock.sh" "$test_root/bin/$mock"
done
for entry in popup app; do
  args=()
  [[ $entry != popup ]] || args=(--ensure-running)
  for outcome in cancel success active; do
    state="$test_root/$entry-$outcome"
    mkdir -p "$state"
    touch "$state/calls"
    [[ $outcome != active ]] || touch "$state/active"
    PATH="$test_root/bin:$PATH" XDG_CONFIG_HOME="$test_root/config" \
      XDG_RUNTIME_DIR="$test_root/runtime" WISP_TEST_STATE="$state" \
      WISP_TEST_OUTCOME="$outcome" timeout 5 "$test_root/bin/wisp" "${args[@]}" >/dev/null
    if [[ $outcome == active ]]; then
      ! rg -q '^prompt:' "$state/calls"
      if [[ $entry == popup ]]; then
        [[ ! -s "$state/calls" ]]
      else
        rg -q '^ui:app open$' "$state/calls"
      fi
    else
      [[ $(rg -c '^prompt:login$' "$state/calls") == 1 ]]
    fi
  done
done
# The non-systemd path must likewise leave an already-running client alone.
PATH="$test_root/bin:$PATH" XDG_CONFIG_HOME="$test_root/config" \
  WISP_TEST_SYSTEMD=no WISP_TEST_STATE="$test_root/popup-active" \
  "$test_root/bin/wisp" --ensure-running
[[ ! -s "$test_root/popup-active/calls" ]]

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
  mkdir -p "$test_root/app" "$test_root/session"
  cp "$repo_dir/quickshell/app/WispSessionLauncher.qml" "$test_root/app/"
  cp "$repo_dir/tests/quickshell/SessionLauncher.qml" "$test_root/shell.qml"
  install -m 0755 "$repo_dir/tests/fixtures/session-launch-mock.sh" "$test_root/bin/wisp"
  PATH="$test_root/bin:$PATH" XDG_CONFIG_HOME="$test_root/config" \
    XDG_RUNTIME_DIR="$test_root/runtime" WISP_TEST_STATE="$test_root/session" \
    QT_QPA_PLATFORM=offscreen QT_QUICK_BACKEND=software \
    timeout 10 qs --path "$test_root" >"$test_root/qml.log" 2>&1 || { cat "$test_root/qml.log"; exit 1; }
  rg -q 'SESSION_LAUNCHER_OK' "$test_root/qml.log"
  [[ $(rg -c '^wisp:--ensure-running$' "$test_root/session/calls") == 1 ]]
  [[ $(rg -c '^wisp:$' "$test_root/session/calls") == 1 ]]
fi
rg -q 'sessionLauncher.ensureRunning\(\)' "$repo_dir/quickshell/Panel.qml"
rg -q 'sessionLauncher.openApp\(\)' "$repo_dir/quickshell/Panel.qml"
echo 'Login/register navigation, Omarchy launch routes, and launch success/cancellation passed'
