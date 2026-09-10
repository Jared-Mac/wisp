#!/usr/bin/env bash
set -euo pipefail
repo_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
test_dir=$(mktemp -d)
trap 'rm -rf -- "$test_dir"' EXIT
mkdir -p "$test_dir/config/wisp"
cp -a "$repo_dir/quickshell/app" "$test_dir/app"
cp "$repo_dir/tests/quickshell/ProfileAndCues.qml" "$test_dir/shell.qml"
XDG_CONFIG_HOME="$test_dir/config" WISP_SOCKET="$test_dir/offline.sock" QT_QPA_PLATFORM=offscreen QT_QUICK_BACKEND=software \
 timeout 12 qs --path "$test_dir" >"$test_dir/log" 2>&1 || { cat "$test_dir/log"; exit 1; }
if rg 'PROFILE_CUES_FAILED|TypeError|ReferenceError|Binding loop|Cannot assign|Failed to load' "$test_dir/log"; then cat "$test_dir/log"; exit 1; fi
rg -q PROFILE_CUES_OK "$test_dir/log"
python3 - "$test_dir/config/wisp/appearance.json" <<'PY'
import json, sys
assert json.load(open(sys.argv[1]))['showAvatars'] is False
PY
echo 'Confirmed audio cues, sound preferences, avatar persistence and cache races passed'
