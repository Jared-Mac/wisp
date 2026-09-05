#!/usr/bin/env bash
set -euo pipefail
repo_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
test_dir=$(mktemp -d /tmp/wisp-privacy-ui.XXXXXX)
fixture_pid=""
cleanup() {
  if [[ -n "$fixture_pid" ]]; then kill "$fixture_pid" 2>/dev/null || true; wait "$fixture_pid" 2>/dev/null || true; fi
  rm -rf -- "$test_dir"
}
trap cleanup EXIT
cp -a "$repo_dir/quickshell/app" "$test_dir/app"
cp "$repo_dir/tests/quickshell/PrivacyRouting.qml" "$test_dir/shell.qml"
mkdir -p "$test_dir/config"
node -e 'require("net").createServer(s => s.on("data", () => {})).listen(process.argv[1])' "$test_dir/fixture.sock" &
fixture_pid=$!
for _ in {1..50}; do [[ -S "$test_dir/fixture.sock" ]] && break; sleep 0.02; done
XDG_CONFIG_HOME="$test_dir/config" WISP_SOCKET="$test_dir/fixture.sock" \
  QT_QPA_PLATFORM=offscreen QT_QUICK_BACKEND=software \
  timeout 10 qs --path "$test_dir" >"$test_dir/log" 2>&1 || { cat "$test_dir/log"; exit 1; }
if rg 'PRIVACY_ROUTING_FAILED|TypeError|ReferenceError|Binding loop|Cannot assign|Failed to load' "$test_dir/log"; then cat "$test_dir/log"; exit 1; fi
rg -q PRIVACY_ROUTING_OK "$test_dir/log"
echo 'Privacy startup ordering, server switching, and stale response checks passed'
