#!/usr/bin/env bash
set -euo pipefail
repo_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
test_dir=$(mktemp -d)
trap 'rm -rf -- "$test_dir"' EXIT
mkdir -p "$test_dir/config/wisp" "$test_dir/bin"
cp -a "$repo_dir/quickshell/app" "$test_dir/app"
cp "$repo_dir/tests/quickshell/RoomActivity.qml" "$test_dir/shell.qml"
cat > "$test_dir/bin/notify-send" <<'SH'
#!/bin/sh
printf '%s\n' "$@" >> "$WISP_NOTIFICATION_CAPTURE"
printf 'default\n'
SH
chmod +x "$test_dir/bin/notify-send"
node "$repo_dir/scripts/test-room-activity.cjs"
for reload in 0 1; do
  PATH="$test_dir/bin:$PATH" WISP_NOTIFICATION_CAPTURE="$test_dir/notifications.log" \
    XDG_CONFIG_HOME="$test_dir/config" WISP_SOCKET="$test_dir/no-daemon.sock" \
    WISP_ROOM_ACTIVITY_RELOAD="$reload" QT_QPA_PLATFORM=offscreen QT_QUICK_BACKEND=software \
    timeout 15 qs --path "$test_dir" > "$test_dir/ui.log" 2>&1 || { cat "$test_dir/ui.log"; exit 1; }
  if rg 'ROOM_ACTIVITY_FAILED|TypeError|ReferenceError|Binding loop|Cannot assign|Cannot anchor|Failed to load' "$test_dir/ui.log"; then cat "$test_dir/ui.log"; exit 1; fi
  rg -q ROOM_ACTIVITY_OK "$test_dir/ui.log" || { cat "$test_dir/ui.log"; exit 1; }
done
test "$(rg -c 'Friends? joined a room' "$test_dir/notifications.log")" = 1
rg -q 'Riley joined Lounge' "$test_dir/notifications.log"
echo 'Home defaults, multiple homes, dropdown buttons and drag, persistence, desktop alert action and no voice joins passed'
