#!/usr/bin/env bash
set -euo pipefail
repo_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
test_dir=$(mktemp -d)
fixture_pid=''
trap 'if [[ -n "$fixture_pid" ]]; then kill "$fixture_pid" 2>/dev/null || true; fi; rm -rf -- "$test_dir"' EXIT
mkdir -p "$test_dir/config/wisp" "$test_dir/app/native/WispVideo"
cp -a "$repo_dir/quickshell/app/." "$test_dir/app/"
cp "$repo_dir/tests/quickshell/StreamsAndSearch.qml" "$test_dir/shell.qml"
cp "$repo_dir/target/video-ui/libwispvideo.so" "$repo_dir/target/video-ui/qmldir" "$test_dir/app/native/WispVideo/"
"${NODE:-node}" "$repo_dir/tests/video-fixture.mjs" "$test_dir/test.video" >"$test_dir/video.log" 2>&1 &
fixture_pid=$!
for _ in {1..40}; do [[ -S "$test_dir/test.video" ]] && break; sleep 0.05; done
[[ -S "$test_dir/test.video" ]] || { cat "$test_dir/video.log"; exit 1; }
XDG_CONFIG_HOME="$test_dir/config" WISP_SOCKET="$test_dir/test.sock" \
  QML_IMPORT_PATH="$test_dir/app/native" QT_QPA_PLATFORM=offscreen QT_QUICK_BACKEND=software \
  timeout 25 qs --path "$test_dir" >"$test_dir/log" 2>&1 || { cat "$test_dir/log"; exit 1; }
if rg 'STREAM_SEARCH_FAILED|TypeError|ReferenceError|Binding loop|Cannot assign|Cannot anchor|Failed to load|Cannot specify' "$test_dir/log"; then cat "$test_dir/log" "$test_dir/video.log"; exit 1; fi
rg -q STREAM_SEARCH_OK "$test_dir/log" || { cat "$test_dir/log"; exit 1; }
echo 'Participant streams, window/tile/leave lifecycle, settings search and scroll targets passed'
