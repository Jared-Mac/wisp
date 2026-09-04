#!/usr/bin/env bash
set -euo pipefail
repo_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
test_dir=$(mktemp -d)
trap 'rm -rf -- "$test_dir"' EXIT
mkdir -p "$test_dir/config/wisp"
cp -a "$repo_dir/quickshell/app" "$test_dir/app"
cp "$repo_dir/tests/quickshell/CameraConfirmation.qml" "$test_dir/shell.qml"
for presentation in app panel; do
  XDG_CONFIG_HOME="$test_dir/config" WISP_SOCKET="$test_dir/missing.sock" \
    WISP_CAMERA_PRESENTATION="$presentation" WISP_CAMERA_SCREENSHOT="${WISP_CAMERA_SCREENSHOT_PREFIX:-/tmp/wisp-camera-confirmation}-$presentation.png" \
    QT_QPA_PLATFORM=offscreen QT_QUICK_BACKEND=software \
    timeout 15 qs --path "$test_dir" >"$test_dir/log" 2>&1 || { cat "$test_dir/log"; exit 1; }
  if ! rg -q CAMERA_CONFIRMATION_OK "$test_dir/log" || rg -q 'CAMERA_TEST_FAILED|TypeError|ReferenceError|Binding loop|Cannot assign|Failed to load' "$test_dir/log"; then cat "$test_dir/log"; exit 1; fi
done
echo 'Camera confirmation, cancel, target-change, and local-preview lifecycle passed without hardware capture'
