#!/usr/bin/env bash
set -euo pipefail

export WISP_E2EE_KEY="wisp-integration-e2ee-key-32-bytes"

repo_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
cd "$repo_dir"

test_dir=$(mktemp -d)
export XDG_CONFIG_HOME="$test_dir/config"
export WISP_ACCOUNTS_FILE="$test_dir/accounts.json"
livekit_pid=""
server_pid=""
daemon_pid=""
member_a_pid=""
member_b_pid=""
member_c_pid=""

stop_process() {
  local signal=$1
  local pid=$2
  [[ -z "$pid" ]] || kill "-$signal" "$pid" 2>/dev/null || true
}

cleanup() {
  local status=$?
  trap - EXIT INT TERM
  for pid in "$daemon_pid" "$member_a_pid" "$member_b_pid" "$member_c_pid"; do
    stop_process INT "$pid"
  done
  sleep 0.1
  for pid in "$daemon_pid" "$member_a_pid" "$member_b_pid" "$member_c_pid" "$server_pid" "$livekit_pid"; do
    stop_process TERM "$pid"
  done
  for pid in "$daemon_pid" "$member_a_pid" "$member_b_pid" "$member_c_pid" "$server_pid" "$livekit_pid"; do
    [[ -z "$pid" ]] || wait "$pid" 2>/dev/null || true
  done
  if [[ $status -ne 0 ]]; then
    for log in "$test_dir"/*.log; do
      [[ -f "$log" ]] || continue
      echo "Last lines of $log:" >&2
      tail -n 40 "$log" >&2
    done
  fi
  rm -rf -- "$test_dir"
  exit "$status"
}
trap cleanup EXIT INT TERM

livekit_port=$(shuf -i 22000-28000 -n 1)
livekit_tcp_port=$((livekit_port + 1))
server_port=$(shuf -i 30000-38000 -n 1)
rtc_port_start=$(shuf -i 42000-58000 -n 1)
rtc_port_end=$((rtc_port_start + 50))
socket_path="$test_dir/wispd.sock"
cycles=${WISP_RELIABILITY_CYCLES:-3}
rss_growth_limit_kib=${WISP_RSS_GROWTH_LIMIT_KIB:-131072}

sed \
  -e "s/port: 7880/port: $livekit_port/" \
  -e "s/tcp_port: 7881/tcp_port: $livekit_tcp_port/" \
  -e "s/port_range_start: 50000/port_range_start: $rtc_port_start/" \
  -e "s/port_range_end: 50100/port_range_end: $rtc_port_end/" \
  infra/local/livekit.yaml >"$test_dir/livekit.yaml"

start_livekit() {
  "$repo_dir/.tools/livekit/livekit-server" --config "$test_dir/livekit.yaml" \
    >>"$test_dir/livekit.log" 2>&1 &
  livekit_pid=$!
  for _ in $(seq 1 100); do
    if ss -ltn | rg -q ":$livekit_port\\b"; then return; fi
    sleep 0.05
  done
  echo "LiveKit did not start on port $livekit_port" >&2
  return 1
}

status_json=""
wait_for_status() {
  local filter=$1
  for _ in $(seq 1 200); do
    status_json=$(target/debug/wispctl --socket "$socket_path" status 2>/dev/null || true)
    if jq -e "$filter" <<<"$status_json" >/dev/null 2>&1; then return; fi
    sleep 0.1
  done
  echo "Timed out waiting for status filter: $filter" >&2
  jq '.' <<<"${status_json:-null}" >&2 || true
  return 1
}

read_rss_kib() {
  awk '/^VmRSS:/ { print $2 }' "/proc/$daemon_pid/status"
}

run_quickshell_probe() {
  local label=$1
  WISP_SOCKET="$socket_path" quickshell --path tests/quickshell/IpcProbe.qml \
    >"$test_dir/quickshell-$label.log" 2>&1
  rg -q 'WISP_IPC_PROBE_OK' "$test_dir/quickshell-$label.log"
}

./scripts/livekit-install.sh >/dev/null
cargo build --workspace
command -v quickshell >/dev/null
start_livekit

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
  target/debug/wisp-sim --profile MemberA --publish-tone >"$test_dir/member_a.log" 2>&1 &
member_a_pid=$!
WISP_SERVER_URL="http://127.0.0.1:$server_port" RUST_LOG=info \
  target/debug/wispd --profile Owner --socket "$socket_path" \
  >"$test_dir/daemon.log" 2>&1 &
daemon_pid=$!
for _ in $(seq 1 200); do
  [[ -S "$socket_path" ]] && break
  sleep 0.05
done
[[ -S "$socket_path" ]]

target/debug/wispctl --socket "$socket_path" join MemberA
wait_for_status '
  .self.connection == "connected" and
  .self.media.livekit_connected == true and
  .self.media.e2ee_enabled == true and
  .self.media.remote_audio_participants == ["MemberA"] and
  .self.media.received_audio_frames > 0 and
  .self.media.audio.preset == "clear" and
  .self.media.audio.denoiser_active == true and
  .self.media.audio.denoiser == "deepfilternet" and
  .self.media.audio.processing_latency_ms == 30 and
  .self.media.audio.deepfilter_strength == 100 and
  (.self.media.audio.processing_time_us | type) == "number" and
  (.self.media.audio.processing_deadline_misses | type) == "number" and
  (.self.media.audio.capture_queue_ms | type) == "number"
'

WISP_SERVER_URL="http://127.0.0.1:$server_port" RUST_LOG=info \
  target/debug/wisp-sim --profile MemberB --join Owner --publish-tone >"$test_dir/member_b.log" 2>&1 &
member_b_pid=$!
wait_for_status '
  .self.media.remote_audio_participants == ["MemberB", "MemberA"] and
  .self.media.received_audio_frames > 0
'

WISP_SERVER_URL="http://127.0.0.1:$server_port" RUST_LOG=info \
  target/debug/wisp-sim --profile MemberC --join Owner --publish-tone >"$test_dir/member_c.log" 2>&1 &
member_c_pid=$!
wait_for_status '
  .self.connection == "connected" and
  .self.media.remote_audio_participants == ["MemberC", "MemberB", "MemberA"] and
  .self.media.received_audio_frames > 0
'

sleep 1
rss_baseline_kib=$(read_rss_kib)

for cycle in $(seq 1 "$cycles"); do
  target/debug/wispctl --socket "$socket_path" leave
  wait_for_status '
    .self.connection == "available" and
    .self.hangout_id == null and
    .self.media.livekit_connected == false and
    .self.media.remote_audio_participants == []
  '
  target/debug/wispctl --socket "$socket_path" join MemberA
  wait_for_status '
    .self.connection == "connected" and
    .self.media.livekit_connected == true and
    .self.media.remote_audio_participants == ["MemberC", "MemberB", "MemberA"] and
    .self.media.received_audio_frames > 0 and
    .self.media.audio.denoiser == "deepfilternet"
  '
  echo "Leave/rejoin cycle $cycle passed."
done

frames_before_ipc=$(jq -r '.self.media.received_audio_frames' <<<"$status_json")
run_quickshell_probe first
run_quickshell_probe restarted
for _ in $(seq 1 20); do
  target/debug/wispctl --socket "$socket_path" status >/dev/null
done
for _ in $(seq 1 100); do
  status_json=$(target/debug/wispctl --socket "$socket_path" status)
  if jq -e --argjson before "$frames_before_ipc" \
    '.self.connection == "connected" and .self.media.received_audio_frames > $before' \
    <<<"$status_json" >/dev/null; then break; fi
  sleep 0.1
done
jq -e --argjson before "$frames_before_ipc" '
  .self.connection == "connected" and
  .self.media.remote_audio_participants == ["MemberC", "MemberB", "MemberA"] and
  .self.media.received_audio_frames > $before
' <<<"$status_json" >/dev/null

frames_before_restart=$(jq -r '.self.media.received_audio_frames' <<<"$status_json")
kill -KILL "$livekit_pid"
wait "$livekit_pid" 2>/dev/null || true
livekit_pid=""
sleep 0.3
start_livekit
for _ in $(seq 1 300); do
  status_json=$(target/debug/wispctl --socket "$socket_path" status)
  if jq -e --argjson before "$frames_before_restart" '
    .self.connection == "connected" and
    .self.media.livekit_connected == true and
    .self.media.remote_audio_participants == ["MemberC", "MemberB", "MemberA"] and
    .self.media.received_audio_frames > $before
  ' <<<"$status_json" >/dev/null; then break; fi
  sleep 0.1
done
jq -e --argjson before "$frames_before_restart" '
  .self.connection == "connected" and
  .self.media.livekit_connected == true and
  .self.media.remote_audio_participants == ["MemberC", "MemberB", "MemberA"] and
  .self.media.received_audio_frames > $before
' <<<"$status_json" >/dev/null
rg -q 'LiveKit media reconnecting' "$test_dir/daemon.log"
rg -q 'LiveKit media reconnected' "$test_dir/daemon.log"

target/debug/wispctl --socket "$socket_path" leave
kill -KILL "$livekit_pid"
wait "$livekit_pid" 2>/dev/null || true
livekit_pid=""
sleep 0.2
if target/debug/wispctl --socket "$socket_path" join MemberA \
  >"$test_dir/expected-connect-error.log" 2>&1; then
  echo "Joining unexpectedly succeeded while LiveKit was offline" >&2
  exit 1
fi
wait_for_status '
  .self.connection == "failed" and
  .self.media.livekit_connected == false and
  .self.media.error_code == "livekit_connection" and
  (.self.media.error | startswith("LiveKit connection failed:"))
'
start_livekit
target/debug/wispctl --socket "$socket_path" join MemberA
wait_for_status '
  .self.connection == "connected" and
  .self.media.livekit_connected == true and
  .self.media.error_code == null and
  .self.media.error == null and
  .self.media.remote_audio_participants == ["MemberC", "MemberB", "MemberA"] and
  .self.media.received_audio_frames > 0
'

rss_final_kib=$(read_rss_kib)
rss_growth_kib=$((rss_final_kib - rss_baseline_kib))
if ((rss_growth_kib < 0)); then rss_growth_kib=0; fi
if ((rss_growth_kib > rss_growth_limit_kib)); then
  echo "wispd RSS grew by ${rss_growth_kib} KiB; limit is ${rss_growth_limit_kib} KiB" >&2
  exit 1
fi

jq '.self.media | {remote_audio_participants, received_audio_frames, error_code, error}' \
  <<<"$status_json"
echo "Four-user voice reliability test passed (${cycles} rejoin cycles, RSS +${rss_growth_kib} KiB)."
