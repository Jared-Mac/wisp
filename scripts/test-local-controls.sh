#!/usr/bin/env bash
set -euo pipefail
repo_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
test_dir=$(mktemp -d)
fixture_pid=""
cleanup() { if [[ -n "$fixture_pid" ]]; then kill "$fixture_pid" 2>/dev/null || true; fi; rm -rf -- "$test_dir"; }
trap cleanup EXIT
bash "$repo_dir/scripts/build-video-ui.sh"
mkdir -p "$test_dir/config" "$test_dir/app"
cp -a "$repo_dir/quickshell/app/." "$test_dir/app/"
mkdir -p "$test_dir/app/native/WispVideo"
cp "$repo_dir/target/video-ui/libwispvideo.so" "$repo_dir/target/video-ui/qmldir" "$test_dir/app/native/WispVideo/"
cp "$repo_dir/tests/quickshell/AttentionMedia.qml" "$test_dir/shell.qml"
"${NODE:-node}" "$repo_dir/tests/video-fixture.mjs" "$test_dir/test.video" &
fixture_pid=$!
for _ in {1..40}; do [[ -S "$test_dir/test.video" ]] && break; sleep 0.05; done
QML_IMPORT_PATH="$test_dir/app/native" XDG_CONFIG_HOME="$test_dir/config" WISP_SOCKET="$test_dir/test.sock" QT_QPA_PLATFORM=offscreen QT_QUICK_BACKEND=software \
  timeout 20 qs --path "$test_dir" >"$test_dir/log" 2>&1 || { cat "$test_dir/log"; exit 1; }
cat "$test_dir/log"
if rg 'LOCAL_TEST_FAILED|TypeError|ReferenceError|Binding loop|Cannot assign|Cannot anchor|Failed to load' "$test_dir/log"; then exit 1; fi
rg -q 'LOCAL_CONTROLS_OK' "$test_dir/log"
jq -e '.accounts.self.charlie == 50 and .accounts.self.jared == 200' "$test_dir/config/wisp/participant-volumes.json" >/dev/null
jq -e '.mutedChats == ["dm"] and (.eventSounds | length) == 4' "$test_dir/config/wisp/notifications.json" >/dev/null
echo 'Unread navigation, local volumes, speaking activity and real native stream tiles passed'
mkdir -p "$test_dir/bin" "$test_dir/sound-config"
install -m 0755 "$repo_dir/tests/fakes/pw-play" "$test_dir/bin/pw-play"
cp "$repo_dir/tests/quickshell/SoundPlayback.qml" "$test_dir/shell.qml"
PATH="$test_dir/bin:$PATH" WISP_TEST_SOUND_LOG="$test_dir/sounds.log" WISP_SOUND_DIR="$test_dir/app/assets" \
  XDG_CONFIG_HOME="$test_dir/sound-config" WISP_SOCKET="$test_dir/test.sock" QT_QPA_PLATFORM=offscreen QT_QUICK_BACKEND=software \
  timeout 10 qs --path "$test_dir" >"$test_dir/sound-log" 2>&1
rg -q SOUND_PLAYBACK_OK "$test_dir/sound-log" || { cat "$test_dir/sound-log"; exit 1; }
[[ $(wc -l < "$test_dir/sounds.log") == 4 ]]
for sound in self_join member_join message self_leave; do rg -q "0.4.*$sound.wav" "$test_dir/sounds.log"; done
echo 'Four room cues, custom file routing, playback queue and global mute passed without playing audio'
