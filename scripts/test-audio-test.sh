#!/usr/bin/env bash
# Local-only generated audio: never opens the user's microphone or joins a room.
set -euo pipefail
repo_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
cd "$repo_dir"
test_dir=$(mktemp -d)
server_pid=""
daemon_pid=""
sink_module=""
recorder_pid=""
cleanup() {
  local status=$?
  trap - EXIT
  for pid in "$recorder_pid" "$daemon_pid" "$server_pid"; do
    if [[ -n "$pid" ]]; then kill "$pid" 2>/dev/null || true; wait "$pid" 2>/dev/null || true; fi
  done
  if [[ -n "$sink_module" ]]; then pactl unload-module "$sink_module" >/dev/null; fi
  if [[ "$status" -ne 0 ]]; then echo "Microphone test logs kept at $test_dir" >&2
  else rm -rf -- "$test_dir"; fi
  return "$status"
}
trap cleanup EXIT
trap 'exit 130' INT TERM
export XDG_CONFIG_HOME="$test_dir/config"
export WISP_ACCOUNTS_FILE="$test_dir/accounts.json"
export WISP_E2EE_KEY="wisp-integration-e2ee-key-32-bytes"
port=$(shuf -i 30000-38000 -n 1)
export WISP_SERVER_URL="http://127.0.0.1:$port"
build_profile=${WISP_TEST_BUILD_PROFILE:-dev}
case "$build_profile" in
  dev) build_dir=debug ;;
  release) build_dir=release ;;
  *) echo 'WISP_TEST_BUILD_PROFILE must be dev or release' >&2; exit 2 ;;
esac
cargo build --profile "$build_profile" -p wispd -p wispctl -p wisp-server
WISP_SERVER_ADDR="127.0.0.1:$port" WISP_DATABASE_URL="sqlite://$test_dir/wisp.sqlite3" \
  "target/$build_dir/wisp-server" >"$test_dir/server.log" 2>&1 &
server_pid=$!
for _ in $(seq 1 100); do
  if curl --silent --fail "$WISP_SERVER_URL/healthz" >/dev/null; then break; fi
  sleep .1
done
curl --silent --fail "$WISP_SERVER_URL/healthz" >/dev/null
WISP_TEST_MICROPHONE_TONE=1 WISP_DISABLE_TRAY=1 \
  "target/$build_dir/wispd" --profile Owner --disable-surfaces --socket "$test_dir/wispd.sock" >"$test_dir/daemon.log" 2>&1 &
daemon_pid=$!
ctl() { "target/$build_dir/wispctl" --socket "$test_dir/wispd.sock" "$@"; }
for _ in $(seq 1 200); do [[ -S "$test_dir/wispd.sock" ]] && break; sleep .1; done
[[ -S "$test_dir/wispd.sock" ]]
for _ in $(seq 1 100); do
  if ctl status | jq -e '.self.connection == "available"' >/dev/null; then break; fi
  sleep .1
done
assert_idle_room() {
  ctl status | jq -e '.self.hangout_id == null and .self.media.microphone_published == false and .self.media.livekit_connected == false' >/dev/null
}
wait_phase() {
  local phase=$1
  for _ in $(seq 1 150); do
    if ctl audio test status | jq -e --arg phase "$phase" '.phase == $phase and .error == null' >/dev/null; then return; fi
    sleep .1
  done
  ctl audio test status >&2
  echo "Microphone test did not reach $phase" >&2
  return 1
}
# End-of-stream alone does not establish that a speaker received sound. Read
# only our isolated virtual sink's monitor, never a real microphone/system mix.
assert_audible_playback() {
  local action=$1
  parec --device="wisp_mic_test_$$.monitor" --format=s16le --rate=48000 --channels=1 \
    --latency-msec=20 --process-time-msec=10 >"$test_dir/playback.pcm" 2>"$test_dir/monitor.log" &
  recorder_pid=$!
  sleep .3
  ctl audio test "$action" | jq -e '.phase == "playing"' >/dev/null
  wait_phase ready
  sleep .15
  kill "$recorder_pid"
  wait "$recorder_pid" || true
  recorder_pid=""
  python3 - "$test_dir/playback.pcm" <<'PYTHON'
import array, sys
pcm = array.array('h')
with open(sys.argv[1], 'rb') as recording:
    pcm.frombytes(recording.read())
peak = max(map(abs, pcm), default=0)
assert peak > 1000, f'Playback finished without audible output (peak={peak})'
PYTHON
}
sink_module=$(pactl load-module module-null-sink "sink_name=wisp_mic_test_$$" sink_properties=device.description=Wisp_Microphone_Test)
output_id=""
for _ in $(seq 1 50); do
  output_id=$(ctl audio devices | jq -r '.output_devices[] | select(.name == "Wisp_Microphone_Test") | .id' | head -n 1)
  [[ -n "$output_id" ]] && break
  sleep .1
done
[[ -n "$output_id" ]]
ctl audio output -- "$output_id" >/dev/null
assert_idle_room
ctl audio test status | jq -e '.phase == "idle" and .duration_ms == 0' >/dev/null
if ctl audio test play_original >"$test_dir/no-sample.out" 2>&1; then echo 'Playback accepted an empty sample' >&2; exit 1; fi
ctl mute >/dev/null
ctl audio preset studio >/dev/null
ctl audio test record | jq -e '.phase == "recording" and .preset == "studio"' >/dev/null
sleep .6
ctl audio test status | jq -e '.duration_ms > 0 and .input_level > 0' >/dev/null
ctl audio test stop | jq -e '.phase == "ready" and .duration_ms > 0 and .duration_ms < 8000' >/dev/null
assert_audible_playback play_original
assert_audible_playback play_processed
ctl audio test play_processed >/dev/null
ctl audio test clear | jq -e '.phase == "idle" and .duration_ms == 0' >/dev/null
ctl status | jq -e '.self.muted == true' >/dev/null
ctl audio preset clear >/dev/null
ctl audio test record >/dev/null
wait_phase ready
ctl audio test status | jq -e '.duration_ms == 8000 and .preset == "clear"' >/dev/null
ctl audio test play_processed >/dev/null
wait_phase ready
assert_idle_room
ctl audio preset natural >/dev/null
ctl audio test status | jq -e '.phase == "idle" and .duration_ms == 0' >/dev/null
ctl audio test record >/dev/null
ctl audio test clear >/dev/null
sleep .2
ctl audio test status | jq -e '.phase == "idle" and .duration_ms == 0' >/dev/null
assert_idle_room
python3 "$repo_dir/tests/soundboard-audio.py" "$test_dir" "wisp_mic_test_$$.monitor"
echo 'Local microphone recording, duration cap, audible output from both playback paths, cancellation, preset reset and room/mute isolation passed.'
