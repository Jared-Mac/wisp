#!/usr/bin/env bash
set -euo pipefail
repo_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
test_dir=$(mktemp -d)
trap 'rm -rf -- "$test_dir"' EXIT
mkdir -p "$test_dir/config/wisp" "$test_dir/bin" "$test_dir/state/wisp" "$test_dir/runtime" "$test_dir/release"
cp -a "$repo_dir/quickshell/app" "$test_dir/app"
cp "$repo_dir/tests/quickshell/ClientUpdates.qml" "$test_dir/shell.qml"
install -m 0755 "$repo_dir/scripts/wisp-updater.py" "$test_dir/bin/wisp-updater"
python3 - "$test_dir" <<'PY'
import json,sys
from pathlib import Path
folder=Path(sys.argv[1])
(folder/'release/wisp-main-linux-x86_64.update.json').write_text(json.dumps(dict(format=1,platform='linux-x86_64',commit='b'*40,commit_time=200,sha256='c'*64,archive='wisp-main-linux-x86_64.tar.gz',size=100)))
(folder/'config/wisp/installed-release.json').write_text(json.dumps(dict(commit='a'*40,commit_time=100)))
PY
XDG_CONFIG_HOME="$test_dir/config" XDG_STATE_HOME="$test_dir/state" XDG_BIN_HOME="$test_dir/bin" XDG_RUNTIME_DIR="$test_dir/runtime" \
  WISP_TEST_LEASE="$test_dir/runtime/lease.json" WISP_UPDATE_BASE_URL="file://$test_dir/release" WISP_SOCKET="$test_dir/no-daemon.sock" \
  QT_QPA_PLATFORM=offscreen QT_QUICK_BACKEND=software timeout 15 qs --path "$test_dir" >"$test_dir/log" 2>&1 || { cat "$test_dir/log"; exit 1; }
if rg 'UPDATES_FAILED|TypeError|ReferenceError|Binding loop|Cannot assign|Failed to load|does not exist|is not defined' "$test_dir/log"; then cat "$test_dir/log"; exit 1; fi
rg -q UPDATES_OK "$test_dir/log" || { cat "$test_dir/log"; exit 1; }
echo 'Update settings, manual checks, draft/voice protection and live connection status passed'
