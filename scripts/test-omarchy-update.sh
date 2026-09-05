#!/usr/bin/env bash
set -euo pipefail
repo_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
test_root=$(mktemp -d)
trap 'rm -rf -- "$test_root"' EXIT
mkdir -p "$test_root/release/bin" "$test_root/release/scripts" "$test_root/release/quickshell/app" "$test_root/mock-bin"
cp "$repo_dir/scripts/install-release.sh" "$test_root/release/install.sh"
install -m 0755 "$repo_dir/scripts/plugin-sync.sh" "$test_root/release/scripts/plugin-sync.sh"
cp "$repo_dir/quickshell/Panel.qml" "$test_root/release/quickshell/"
cp "$repo_dir/quickshell/app/WispSessionLauncher.qml" "$test_root/release/quickshell/app/"
mock="$repo_dir/tests/fixtures/omarchy-update-mock.sh"
for name in wispd wisp-account wispctl wisp-server; do
  install -m 0755 "$mock" "$test_root/release/bin/$name"
done
for name in app-sync.sh backup-database.sh restore-database.sh wisp-update.sh; do
  install -m 0755 "$mock" "$test_root/release/scripts/$name"
done
for name in omarchy omarchy-shell; do
  install -m 0755 "$mock" "$test_root/mock-bin/$name"
done
for mode in absent existing; do
  config="$test_root/$mode/config"
  state="$test_root/$mode/state"
  plugin="$config/omarchy/plugins/dev.wisp"
  mkdir -p "$config" "$state"
  if [[ $mode == existing ]]; then
    mkdir -p "$plugin"
    touch "$plugin/previous-version"
  fi
  PATH="$test_root/mock-bin:$PATH" XDG_CONFIG_HOME="$config" \
    XDG_STATE_HOME="$state" XDG_BIN_HOME="$test_root/$mode/bin" \
    WISP_TEST_STATE="$state" bash "$test_root/release/install.sh" >/dev/null
  [[ -f "$state/app-synced" ]]
  if [[ $mode == absent ]]; then
    [[ ! -e "$plugin" && ! -e "$state/rescanned" ]]
  else
    cmp "$repo_dir/quickshell/Panel.qml" "$plugin/Panel.qml"
    cmp "$repo_dir/quickshell/app/WispSessionLauncher.qml" "$plugin/app/WispSessionLauncher.qml"
    [[ -f "$state/rescanned" ]]
    [[ $(find "$state/wisp/backups" -name previous-version | wc -l) == 1 ]]
  fi
done
echo 'Existing Omarchy adapter updated with backup; absent/disabled integration stays absent/disabled'
