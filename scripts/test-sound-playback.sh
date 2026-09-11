#!/usr/bin/env bash
set -euo pipefail
repo_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
test_dir=$(mktemp -d)
trap 'rm -rf -- "$test_dir"' EXIT
mkdir -p "$test_dir/bin" "$test_dir/config"
cp -a "$repo_dir/quickshell/app" "$test_dir/app"
cp "$repo_dir/tests/quickshell/SoundPlayback.qml" "$test_dir/shell.qml"
install -m 0755 "$repo_dir/tests/fakes/pw-play" "$test_dir/bin/pw-play"
PATH="$test_dir/bin:$PATH" WISP_TEST_SOUND_LOG="$test_dir/sounds.log" WISP_SOUND_DIR="$test_dir/app/assets" \
  XDG_CONFIG_HOME="$test_dir/config" WISP_SOCKET="$test_dir/no-daemon.sock" QT_QPA_PLATFORM=offscreen QT_QUICK_BACKEND=software \
  timeout 10 qs --path "$test_dir" >"$test_dir/log" 2>&1 || { cat "$test_dir/log"; exit 1; }
if rg 'SOUND_TEST_FAILED|TypeError|ReferenceError|Binding loop|Failed to load' "$test_dir/log"; then cat "$test_dir/log"; exit 1; fi
rg -q SOUND_PLAYBACK_OK "$test_dir/log" || { cat "$test_dir/log"; exit 1; }
[[ $(wc -l < "$test_dir/sounds.log") == 8 ]]
for sound in self_join member_join message self_leave screen_share_start screen_share_stop stream_viewer_join stream_viewer_leave; do rg -q "0.4.*$sound.wav" "$test_dir/sounds.log"; done
echo 'Room and screen-share cues, queue, custom routing and mute controls passed without playing audio'
