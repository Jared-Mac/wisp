#!/usr/bin/env bash
set -euo pipefail

repo_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
cd "$repo_dir"

test_dir=$(mktemp -d)
livekit_pid=""
server_pid=""
daemon_pid=""
viewer_pid=""
sim_pid=""
expect_surface=true
surface_args=()
if [[ -z "${WAYLAND_DISPLAY:-}" || -z "${XDG_RUNTIME_DIR:-}" ]]; then
  expect_surface=false
  surface_args=(--disable-surfaces)
fi

stop_process() {
  local signal=$1
  local pid=$2
  [[ -z "$pid" ]] || kill "-$signal" "$pid" 2>/dev/null || true
}

cleanup() {
  local status=$?
  trap - EXIT INT TERM
  stop_process INT "$daemon_pid"
  stop_process INT "$viewer_pid"
  stop_process INT "$sim_pid"
  for _ in $(seq 1 50); do
    if { [[ -z "$daemon_pid" ]] || ! kill -0 "$daemon_pid" 2>/dev/null; } &&
       { [[ -z "$viewer_pid" ]] || ! kill -0 "$viewer_pid" 2>/dev/null; } &&
       { [[ -z "$sim_pid" ]] || ! kill -0 "$sim_pid" 2>/dev/null; }; then
      break
    fi
    sleep 0.05
  done
  stop_process TERM "$daemon_pid"
  stop_process TERM "$viewer_pid"
  stop_process TERM "$sim_pid"
  stop_process TERM "$server_pid"
  stop_process TERM "$livekit_pid"
  for pid in "$daemon_pid" "$viewer_pid" "$sim_pid" "$server_pid" "$livekit_pid"; do
    [[ -z "$pid" ]] || wait "$pid" 2>/dev/null || true
  done
  if [[ "${WISP_KEEP_TEST_LOGS:-0}" == "1" || "$status" -ne 0 ]]; then
    echo "Media test logs kept at $test_dir" >&2
  else
    rm -rf -- "$test_dir"
  fi
  return "$status"
}
trap cleanup EXIT INT TERM

livekit_port=$(shuf -i 22000-28000 -n 1)
livekit_tcp_port=$((livekit_port + 1))
server_port=$(shuf -i 30000-38000 -n 1)
rtc_port_start=$(shuf -i 42000-58000 -n 1)
rtc_port_end=$((rtc_port_start + 50))

sed \
  -e "s/port: 7880/port: $livekit_port/" \
  -e "s/tcp_port: 7881/tcp_port: $livekit_tcp_port/" \
  -e "s/port_range_start: 50000/port_range_start: $rtc_port_start/" \
  -e "s/port_range_end: 50100/port_range_end: $rtc_port_end/" \
  infra/local/livekit.yaml >"$test_dir/livekit.yaml"

./scripts/livekit-install.sh >/dev/null
cargo build --workspace

"$repo_dir/.tools/livekit/livekit-server" --node-ip 127.0.0.1 \
  --config "$test_dir/livekit.yaml" \
  >"$test_dir/livekit.log" 2>&1 &
livekit_pid=$!
for _ in $(seq 1 200); do
  if ss -ltn | rg -q ":$livekit_port\\b"; then break; fi
  sleep 0.05
done
ss -ltn | rg -q ":$livekit_port\\b"

WISP_SERVER_ADDR="127.0.0.1:$server_port" \
WISP_DATABASE_URL="sqlite://$test_dir/wisp.sqlite3" \
WISP_LIVEKIT_URL="ws://127.0.0.1:$livekit_port" \
RUST_LOG=info \
target/debug/wisp-server >"$test_dir/server.log" 2>&1 &
server_pid=$!
for _ in $(seq 1 100); do
  if curl --silent --fail "http://127.0.0.1:$server_port/healthz" >/dev/null; then break; fi
  sleep 0.05
done
curl --silent --fail "http://127.0.0.1:$server_port/healthz" >/dev/null

WISP_SERVER_URL="http://127.0.0.1:$server_port" RUST_LOG=info \
  target/debug/wisp-sim --profile Tyler --publish-tone --publish-video --publish-camera >"$test_dir/sim.log" 2>&1 &
sim_pid=$!
WISP_SERVER_URL="http://127.0.0.1:$server_port" WISP_PTT_LEASE_MS=400 \
  WISP_TEST_MICROPHONE_TONE=1 RUST_LOG=info \
  target/debug/wispd --profile Jared --socket "$test_dir/wispd.sock" \
  "${surface_args[@]}" \
  >"$test_dir/daemon.log" 2>&1 &
daemon_pid=$!

for _ in $(seq 1 200); do
  [[ -S "$test_dir/wispd.sock" ]] && break
  sleep 0.05
done
[[ -S "$test_dir/wispd.sock" ]]

target/debug/wispctl --socket "$test_dir/wispd.sock" join Tyler
status_json=""
for _ in $(seq 1 200); do
  status_json=$(target/debug/wispctl --socket "$test_dir/wispd.sock" status)
  if jq -e '
    .self.connection == "connected" and
    .self.media.livekit_connected == true and
    .self.media.microphone_published == true and
    .self.media.remote_audio_participants == ["Tyler"] and
    .self.media.received_video_frames == 0 and
    .self.media.rendered_video_frames == 0 and
    .self.media.surface_open == false and
    .self.media.remote_video_participants == ["Tyler"] and
    (.self.media.remote_videos | length) == 2 and
    ([.self.media.remote_videos[].source] | sort) == ["camera", "screen_share"] and
    ([.self.media.remote_videos[].subscribed] | all(. == false)) and
    ([.self.media.remote_videos[].received_frames] | all(. == 0)) and
    .self.media.last_video_from == null and
    .self.media.video.quality == "high" and
    .self.media.video.codec == "h264" and
    (.self.media.video.encoder_backend | type) == "string" and
    (.self.media.active_speakers | type) == "array" and
    .self.push_to_talk.enabled == false and
    .self.push_to_talk.active == false and
    (.self.media.audio.input_devices | length) > 0 and
    (.self.media.audio.output_devices | length) > 0 and
    (.self.media.audio.selected_input_id | type) == "string" and
    (.self.media.audio.selected_output_id | type) == "string" and
    .self.media.audio.preset == "clear" and
    .self.media.audio.denoiser_active == true and
    .self.media.audio.denoiser == "deepfilternet" and
    .self.media.audio.processing_latency_ms == 30 and
    .self.media.audio.input_level > 0 and
    .self.media.audio.input_level <= 100
  ' <<<"$status_json" >/dev/null; then
    break
  fi
  sleep 0.1
done
jq -e '
  .self.connection == "connected" and
  .self.media.livekit_connected == true and
  .self.media.microphone_published == true and
  .self.media.remote_audio_participants == ["Tyler"] and
  .self.media.received_video_frames == 0 and
  .self.media.rendered_video_frames == 0 and
  .self.media.surface_open == false and
  .self.media.remote_video_participants == ["Tyler"] and
  (.self.media.remote_videos | length) == 2 and
  ([.self.media.remote_videos[].source] | sort) == ["camera", "screen_share"] and
  ([.self.media.remote_videos[].subscribed] | all(. == false)) and
  ([.self.media.remote_videos[].received_frames] | all(. == 0)) and
  .self.media.last_video_from == null and
  .self.media.video.quality == "high" and
  .self.media.video.codec == "h264" and
  (.self.media.active_speakers | type) == "array" and
  .self.push_to_talk.enabled == false and
  .self.push_to_talk.active == false and
  (.self.media.audio.input_devices | length) > 0 and
  (.self.media.audio.output_devices | length) > 0 and
  (.self.media.audio.selected_input_id | type) == "string" and
  (.self.media.audio.selected_output_id | type) == "string" and
  .self.media.audio.preset == "clear" and
  .self.media.audio.denoiser_active == true and
  .self.media.audio.denoiser == "deepfilternet" and
  .self.media.audio.processing_latency_ms == 30 and
  .self.media.audio.input_level > 0 and
  .self.media.audio.input_level <= 100
' <<<"$status_json" >/dev/null || {
  echo "initial M3 media state did not settle" >&2
  jq '.self | {connection, push_to_talk, media}' <<<"$status_json" >&2
  exit 1
}

video_devices_json=$(target/debug/wispctl --socket "$test_dir/wispd.sock" video devices)
jq -e '(.devices | type) == "array"' <<<"$video_devices_json" >/dev/null
if jq -e '(.devices | length) == 0' <<<"$video_devices_json" >/dev/null; then
  if target/debug/wispctl --socket "$test_dir/wispd.sock" camera on \
    >"$test_dir/camera-missing.out" 2>"$test_dir/camera-missing.err"; then
    echo "camera unexpectedly started without a capture device" >&2
    exit 1
  fi
  target/debug/wispctl --socket "$test_dir/wispd.sock" status \
    | jq -e '
      .self.media.camera.active == false and
      .self.media.camera.starting == false and
      (.self.media.camera.error | type) == "string"
    ' >/dev/null
  target/debug/wispctl --socket "$test_dir/wispd.sock" camera off >/dev/null
  target/debug/wispctl --socket "$test_dir/wispd.sock" status \
    | jq -e '.self.media.camera.active == false and .self.media.camera.error == null' >/dev/null
fi
target/debug/wispctl --socket "$test_dir/wispd.sock" video quality balanced \
  | jq -e '.quality == "balanced"' >/dev/null
target/debug/wispctl --socket "$test_dir/wispd.sock" video codec av1 \
  | jq -e '.codec == "av1"' >/dev/null
target/debug/wispctl --socket "$test_dir/wispd.sock" video quality high \
  | jq -e '.quality == "high"' >/dev/null
target/debug/wispctl --socket "$test_dir/wispd.sock" video codec h264 \
  | jq -e '.codec == "h264"' >/dev/null

WISP_SERVER_URL="http://127.0.0.1:$server_port" WISP_DISABLE_SURFACES=true \
  WISP_TEST_MICROPHONE_TONE=1 WISP_DISABLE_TRAY=1 RUST_LOG=info \
  target/debug/wispd --profile Jack --socket "$test_dir/viewer.sock" \
  >"$test_dir/viewer.log" 2>&1 &
viewer_pid=$!
for _ in $(seq 1 200); do
  [[ -S "$test_dir/viewer.sock" ]] && break
  sleep 0.05
done
[[ -S "$test_dir/viewer.sock" ]]
target/debug/wispctl --socket "$test_dir/viewer.sock" join Tyler
viewer_hidden_json=""
for _ in $(seq 1 100); do
  viewer_hidden_json=$(target/debug/wispctl --socket "$test_dir/viewer.sock" status)
  if jq -e '
    .self.connection == "connected" and
    (.self.media.remote_videos | length) == 2 and
    .self.media.received_video_frames == 0 and
    ([.self.media.remote_videos[].subscribed] | all(. == false))
  ' <<<"$viewer_hidden_json" >/dev/null; then
    break
  fi
  sleep 0.1
done
jq -e '
  .self.connection == "connected" and
  (.self.media.remote_videos | length) == 2 and
  .self.media.received_video_frames == 0 and
  ([.self.media.remote_videos[].subscribed] | all(. == false))
' <<<"$viewer_hidden_json" >/dev/null

target/debug/wispctl --socket "$test_dir/wispd.sock" watch Tyler screen_share \
  | jq -e '.watched == true and .source == "screen_share"' >/dev/null
target/debug/wispctl --socket "$test_dir/viewer.sock" watch Tyler screen_share \
  | jq -e '.watched == true and .source == "screen_share"' >/dev/null
watched_json=""
for _ in $(seq 1 100); do
  watched_json=$(target/debug/wispctl --socket "$test_dir/wispd.sock" status)
  if jq -e --argjson expect_surface "$expect_surface" '
    .self.connection == "connected" and
    .self.media.surface_open == $expect_surface and
    (if $expect_surface then .self.media.rendered_video_frames > 0
     else .self.media.rendered_video_frames == 0 end) and
    .self.media.remote_video_participants == ["Tyler"] and
    any(.self.media.remote_videos[];
      .source == "screen_share" and .subscribed and .received_frames > 0) and
    any(.self.media.remote_videos[];
      .source == "camera" and (.subscribed | not) and .received_frames == 0)
  ' <<<"$watched_json" >/dev/null; then
    break
  fi
  sleep 0.1
done
jq -e --argjson expect_surface "$expect_surface" '
  .self.connection == "connected" and
  .self.media.surface_open == $expect_surface and
  (if $expect_surface then .self.media.rendered_video_frames > 0
   else .self.media.rendered_video_frames == 0 end) and
  .self.media.remote_video_participants == ["Tyler"] and
  any(.self.media.remote_videos[];
    .source == "screen_share" and .subscribed and .received_frames > 0) and
  any(.self.media.remote_videos[];
    .source == "camera" and (.subscribed | not) and .received_frames == 0)
' <<<"$watched_json" >/dev/null

target/debug/wispctl --socket "$test_dir/wispd.sock" watch Tyler camera \
  | jq -e '.watched == true and .source == "camera"' >/dev/null
target/debug/wispctl --socket "$test_dir/viewer.sock" watch Tyler camera \
  | jq -e '.watched == true and .source == "camera"' >/dev/null
for _ in $(seq 1 100); do
  watched_json=$(target/debug/wispctl --socket "$test_dir/wispd.sock" status)
  viewer_watched_json=$(target/debug/wispctl --socket "$test_dir/viewer.sock" status)
  if jq -e --argjson expect_surface "$expect_surface" '
      ([.self.media.remote_videos[] | select(.subscribed and .received_frames > 0)] | length) == 2 and
      .self.media.surface_open == $expect_surface and
      (if $expect_surface then .self.media.rendered_video_frames > 0
       else .self.media.rendered_video_frames == 0 end)
    ' <<<"$watched_json" >/dev/null \
    && jq -e '
      ([.self.media.remote_videos[] | select(.subscribed and .received_frames > 0)] | length) == 2 and
      .self.media.surface_open == false
    ' <<<"$viewer_watched_json" >/dev/null; then
    break
  fi
  sleep 0.1
done
jq -e --argjson expect_surface "$expect_surface" '
  ([.self.media.remote_videos[] | select(.subscribed and .received_frames > 0)] | length) == 2 and
  .self.media.surface_open == $expect_surface
' \
  <<<"$watched_json" >/dev/null
jq -e '
  ([.self.media.remote_videos[] | select(.subscribed and .received_frames > 0)] | length) == 2 and
  .self.media.surface_open == false
' <<<"$viewer_watched_json" >/dev/null
status_json=$watched_json

for _ in $(seq 1 100); do
  if rg -q 'simulator received remote audio frames' "$test_dir/sim.log"; then break; fi
  sleep 0.05
done
rg -q 'simulator received remote audio frames' "$test_dir/sim.log"

for _ in $(seq 1 100); do
  if rg -q 'simulator received nonzero remote audio' "$test_dir/sim.log"; then break; fi
  sleep 0.05
done
rg -q 'simulator received nonzero remote audio' "$test_dir/sim.log"

audio_json=$(target/debug/wispctl --socket "$test_dir/wispd.sock" audio devices)
jq -e '
  (.input_devices | length) > 0 and
  (.output_devices | length) > 0 and
  ([.input_devices[].id] | all(. != "") and length == (unique | length)) and
  ([.output_devices[].id] | all(. != "") and length == (unique | length)) and
  (.selected_input_id | type) == "string" and
  (.selected_output_id | type) == "string"
' <<<"$audio_json" >/dev/null
input_id=$(jq -r '.selected_input_id' <<<"$audio_json")
output_id=$(jq -r '.selected_output_id' <<<"$audio_json")
alternate_input_id=$(jq -r '. as $root | [.input_devices[].id | select(. != $root.selected_input_id)][0] // .selected_input_id' <<<"$audio_json")
alternate_output_id=$(jq -r '. as $root | [.output_devices[].id | select(. != $root.selected_output_id)][0] // .selected_output_id' <<<"$audio_json")
target/debug/wispctl --socket "$test_dir/wispd.sock" audio input -- "$alternate_input_id" \
  | jq -e --arg id "$alternate_input_id" '.selected_input_id == $id' >/dev/null
target/debug/wispctl --socket "$test_dir/wispd.sock" audio input -- "$input_id" \
  | jq -e --arg id "$input_id" '.selected_input_id == $id' >/dev/null
target/debug/wispctl --socket "$test_dir/wispd.sock" audio output -- "$alternate_output_id" \
  | jq -e --arg id "$alternate_output_id" '.selected_output_id == $id' >/dev/null
target/debug/wispctl --socket "$test_dir/wispd.sock" audio output -- "$output_id" \
  | jq -e --arg id "$output_id" '.selected_output_id == $id' >/dev/null
target/debug/wispctl --socket "$test_dir/wispd.sock" audio preset studio \
  | jq -e '
    .preset == "studio" and
    .denoiser_active == false and
    .denoiser == null and
    .processing_latency_ms == 0
  ' >/dev/null
target/debug/wispctl --socket "$test_dir/wispd.sock" audio preset clear \
  | jq -e '
    .preset == "clear" and
    .denoiser_active == true and
    .denoiser == "deepfilternet" and
    .processing_latency_ms == 30
  ' >/dev/null

target/debug/wispctl --socket "$test_dir/wispd.sock" ptt enable \
  | jq -e '
    .push_to_talk.enabled == true and
    .push_to_talk.active == false and
    .effective_muted == true
  ' >/dev/null
target/debug/wispctl --socket "$test_dir/wispd.sock" ptt press \
  | jq -e '
    .push_to_talk.active == true and
    .effective_muted == false and
    .blocked_by_mute == false
  ' >/dev/null
target/debug/wispctl --socket "$test_dir/wispd.sock" mute \
  | jq -e '
    .muted == true and
    .push_to_talk.active == false and
    .effective_muted == true
  ' >/dev/null
target/debug/wispctl --socket "$test_dir/wispd.sock" ptt press \
  | jq -e '
    .push_to_talk.active == false and
    .effective_muted == true and
    .blocked_by_mute == true
  ' >/dev/null
target/debug/wispctl --socket "$test_dir/wispd.sock" unmute \
  | jq -e '
    .muted == false and
    .push_to_talk.active == false and
    .effective_muted == true
  ' >/dev/null
target/debug/wispctl --socket "$test_dir/wispd.sock" ptt press \
  | jq -e '.push_to_talk.active == true' >/dev/null
sleep 0.25
target/debug/wispctl --socket "$test_dir/wispd.sock" ptt press \
  | jq -e '.push_to_talk.active == true' >/dev/null
sleep 0.25
target/debug/wispctl --socket "$test_dir/wispd.sock" status \
  | jq -e '.self.push_to_talk.active == true' >/dev/null
target/debug/wispctl --socket "$test_dir/wispd.sock" ptt release \
  | jq -e '.push_to_talk.active == false and .effective_muted == true' >/dev/null
target/debug/wispctl --socket "$test_dir/wispd.sock" ptt press \
  | jq -e '.push_to_talk.active == true' >/dev/null
expired_json=""
for _ in $(seq 1 30); do
  expired_json=$(target/debug/wispctl --socket "$test_dir/wispd.sock" status)
  if jq -e '.self.push_to_talk.active == false' <<<"$expired_json" >/dev/null; then
    break
  fi
  sleep 0.05
done
jq -e '
  .self.push_to_talk.enabled == true and
  .self.push_to_talk.active == false and
  .self.media.audio.input_level == 0
' <<<"$expired_json" >/dev/null
target/debug/wispctl --socket "$test_dir/wispd.sock" ptt disable \
  | jq -e '
    .push_to_talk.enabled == false and
    .push_to_talk.active == false and
    .effective_muted == false
  ' >/dev/null
if target/debug/wispctl --socket "$test_dir/wispd.sock" ptt press \
  >"$test_dir/ptt-disabled.out" 2>"$test_dir/ptt-disabled.err"; then
  echo "PTT press unexpectedly succeeded while disabled" >&2
  exit 1
fi
rg -q 'push-to-talk is disabled' "$test_dir/ptt-disabled.err"

if command -v hyprctl >/dev/null && [[ -n "${WAYLAND_DISPLAY:-}" ]]; then
  hyprctl clients -j | jq -e \
    '[.[] | select(.class == "dev.wisp.surface")] | length >= 2' >/dev/null
fi

audio_markers_before_close=$(rg -c 'simulator audio still flowing' "$test_dir/sim.log" || true)
target/debug/wispctl --socket "$test_dir/wispd.sock" unwatch Tyler screen_share \
  | jq -e '.watched == false' >/dev/null
target/debug/wispctl --socket "$test_dir/wispd.sock" unwatch Tyler camera \
  | jq -e '.watched == false' >/dev/null
closed_json=""
for _ in $(seq 1 100); do
  closed_json=$(target/debug/wispctl --socket "$test_dir/wispd.sock" status)
  if jq -e '
    .self.connection == "connected" and
    .self.media.livekit_connected == true and
    .self.media.surface_open == false and
    ([.self.media.remote_videos[].subscribed] | all(. == false))
  ' <<<"$closed_json" >/dev/null; then
    break
  fi
  sleep 0.1
done
jq -e '
  .self.connection == "connected" and
  .self.media.livekit_connected == true and
  .self.media.surface_open == false and
  ([.self.media.remote_videos[].subscribed] | all(. == false))
' <<<"$closed_json" >/dev/null
for _ in $(seq 1 100); do
  current_audio_markers=$(rg -c 'simulator audio still flowing' "$test_dir/sim.log" || true)
  if (( current_audio_markers > audio_markers_before_close )); then break; fi
  sleep 0.1
done
(( current_audio_markers > audio_markers_before_close ))
hidden_video_frames=$(jq -r '.self.media.received_video_frames' <<<"$closed_json")
hidden_stable_checks=0
hidden_settled_json=$closed_json
for _ in $(seq 1 30); do
  sleep 0.1
  hidden_settled_json=$(target/debug/wispctl --socket "$test_dir/wispd.sock" status)
  current_hidden_frames=$(jq -r '.self.media.received_video_frames' <<<"$hidden_settled_json")
  if [[ "$current_hidden_frames" == "$hidden_video_frames" ]]; then
    hidden_stable_checks=$((hidden_stable_checks + 1))
    if (( hidden_stable_checks >= 3 )); then break; fi
  else
    hidden_video_frames=$current_hidden_frames
    hidden_stable_checks=0
  fi
done
jq -e '([.self.media.remote_videos[].subscribed] | all(. == false))' \
  <<<"$hidden_settled_json" >/dev/null
(( hidden_stable_checks >= 3 ))

target/debug/wispctl --socket "$test_dir/wispd.sock" watch Tyler screen_share \
  | jq -e '.watched == true' >/dev/null
reopened_json=""
for _ in $(seq 1 100); do
  reopened_json=$(target/debug/wispctl --socket "$test_dir/wispd.sock" status)
  if jq -e --argjson expect_surface "$expect_surface" '
    .self.connection == "connected" and
    .self.media.surface_open == $expect_surface and
    (if $expect_surface then .self.media.rendered_video_frames > 0
     else .self.media.rendered_video_frames == 0 end) and
    any(.self.media.remote_videos[];
      .source == "screen_share" and .subscribed and .received_frames > 0) and
    any(.self.media.remote_videos[];
      .source == "camera" and (.subscribed | not))
  ' <<<"$reopened_json" >/dev/null; then
    break
  fi
  sleep 0.1
done
jq -e --argjson expect_surface "$expect_surface" '
  .self.connection == "connected" and
  .self.media.surface_open == $expect_surface and
  (if $expect_surface then .self.media.rendered_video_frames > 0
   else .self.media.rendered_video_frames == 0 end) and
  any(.self.media.remote_videos[];
    .source == "screen_share" and .subscribed and .received_frames > 0) and
  any(.self.media.remote_videos[];
    .source == "camera" and (.subscribed | not))
' <<<"$reopened_json" >/dev/null

target/debug/wispctl --socket "$test_dir/wispd.sock" mute \
  | jq -e '.muted == true' >/dev/null
target/debug/wispctl --socket "$test_dir/wispd.sock" deafen \
  | jq -e '.deafened == true' >/dev/null
target/debug/wispctl --socket "$test_dir/wispd.sock" unmute \
  | jq -e '.muted == false' >/dev/null
target/debug/wispctl --socket "$test_dir/wispd.sock" undeafen \
  | jq -e '.deafened == false' >/dev/null

pre_restart_json=$(target/debug/wispctl --socket "$test_dir/wispd.sock" status)
video_frames_before_restart=$(jq -r '.self.media.received_video_frames' <<<"$pre_restart_json")
audio_markers_before_restart=$(rg -c 'simulator audio still flowing' "$test_dir/sim.log" || true)
kill -KILL "$livekit_pid"
wait "$livekit_pid" 2>/dev/null || true
livekit_pid=""
sleep 0.3

"$repo_dir/.tools/livekit/livekit-server" --node-ip 127.0.0.1 \
  --config "$test_dir/livekit.yaml" \
  >>"$test_dir/livekit.log" 2>&1 &
livekit_pid=$!
for _ in $(seq 1 100); do
  if ss -ltn | rg -q ":$livekit_port\\b"; then break; fi
  sleep 0.05
done
ss -ltn | rg -q ":$livekit_port\\b"

reconnected_json=""
for _ in $(seq 1 200); do
  reconnected_json=$(target/debug/wispctl --socket "$test_dir/wispd.sock" status)
  if jq -e --argjson video_before "$video_frames_before_restart" '
    .self.connection == "connected" and
    .self.media.livekit_connected == true and
    .self.media.received_video_frames > $video_before and
    .self.media.remote_video_participants == ["Tyler"] and
    any(.self.media.remote_videos[];
      .source == "screen_share" and .subscribed and .received_frames > 0) and
    any(.self.media.remote_videos[];
      .source == "camera" and (.subscribed | not))
  ' <<<"$reconnected_json" >/dev/null; then
    break
  fi
  sleep 0.1
done
jq -e --argjson video_before "$video_frames_before_restart" '
  .self.connection == "connected" and
  .self.media.livekit_connected == true and
  .self.media.received_video_frames > $video_before and
  .self.media.remote_video_participants == ["Tyler"] and
  any(.self.media.remote_videos[];
    .source == "screen_share" and .subscribed and .received_frames > 0) and
  any(.self.media.remote_videos[];
    .source == "camera" and (.subscribed | not))
' <<<"$reconnected_json" >/dev/null
for _ in $(seq 1 200); do
  current_audio_markers=$(rg -c 'simulator audio still flowing' "$test_dir/sim.log" || true)
  if (( current_audio_markers > audio_markers_before_restart )); then break; fi
  sleep 0.1
done
(( current_audio_markers > audio_markers_before_restart ))
rg -q 'LiveKit media reconnecting' "$test_dir/daemon.log"
rg -q 'LiveKit media reconnected' "$test_dir/daemon.log"

target/debug/wispctl --socket "$test_dir/wispd.sock" leave
final_json=$(target/debug/wispctl --socket "$test_dir/wispd.sock" status)
jq -e '
  .self.connection == "available" and
  .self.hangout_id == null and
  .self.media.livekit_connected == false
' <<<"$final_json" >/dev/null

jq '.self.media' <<<"$status_json"
echo "LiveKit media integration test passed."
