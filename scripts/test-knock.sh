#!/usr/bin/env bash
set -euo pipefail

repo_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
cd "$repo_dir"

test_dir=$(mktemp -d)
export XDG_CONFIG_HOME="$test_dir/config"
export WISP_ACCOUNTS_FILE="$test_dir/accounts.json"
server_pid=""
owner_pid=""
member_pid=""
sim_pid=""

stop_process() {
  local signal=$1
  local pid=$2
  [[ -z "$pid" ]] || kill "-$signal" "$pid" 2>/dev/null || true
}

cleanup() {
  local status=$?
  trap - EXIT INT TERM
  stop_process INT "$owner_pid"
  stop_process INT "$member_pid"
  stop_process INT "$sim_pid"
  sleep 0.1
  stop_process TERM "$owner_pid"
  stop_process TERM "$member_pid"
  stop_process TERM "$sim_pid"
  stop_process TERM "$server_pid"
  for pid in "$owner_pid" "$member_pid" "$sim_pid" "$server_pid"; do
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

port=$(shuf -i 20000-45000 -n 1)
server_url="http://127.0.0.1:$port"
owner_socket="$test_dir/owner.sock"
member_socket="$test_dir/member_a.sock"

wait_for_status() {
  local socket=$1
  local filter=$2
  local current=""
  for _ in $(seq 1 150); do
    current=$(target/debug/wispctl --socket "$socket" status 2>/dev/null || true)
    if jq -e "$filter" <<<"$current" >/dev/null 2>&1; then
      status_json=$current
      return
    fi
    sleep 0.05
  done
  echo "Timed out waiting for status filter: $filter" >&2
  jq '.' <<<"${current:-null}" >&2 || true
  return 1
}

start_owner() {
  WISP_SERVER_URL="$server_url" RUST_LOG=info \
    target/debug/wispd --profile Owner --socket "$owner_socket" --disable-media \
    >>"$test_dir/owner.log" 2>&1 &
  owner_pid=$!
}

start_member() {
  WISP_SERVER_URL="$server_url" RUST_LOG=info \
    target/debug/wispd --profile MemberA --socket "$member_socket" --disable-media \
    >>"$test_dir/member_a.log" 2>&1 &
  member_pid=$!
}

cargo build --workspace

WISP_SERVER_ADDR="127.0.0.1:$port" \
WISP_DATABASE_URL="sqlite://$test_dir/wisp.sqlite3" \
WISP_KNOCK_TTL_SECONDS=3 \
RUST_LOG=info \
target/debug/wisp-server >"$test_dir/server.log" 2>&1 &
server_pid=$!
for _ in $(seq 1 100); do
  if curl --silent --fail "$server_url/healthz" >/dev/null; then break; fi
  sleep 0.05
done
curl --silent --fail "$server_url/healthz" >/dev/null

start_owner
start_member
for _ in $(seq 1 100); do
  [[ -S "$owner_socket" && -S "$member_socket" ]] && break
  sleep 0.05
done
[[ -S "$owner_socket" && -S "$member_socket" ]]

target/debug/wispctl --socket "$member_socket" presence knock >/dev/null
wait_for_status "$owner_socket" '
  any(.friends[]; .display_name == "MemberA" and .online == true and .presence == "knock")
'

first_knock=$(target/debug/wispctl --socket "$owner_socket" join MemberA)
first_id=$(jq -r 'select(.status == "knock_sent") | .knock_id' <<<"$first_knock")
[[ -n "$first_id" && "$first_id" != "null" ]]
wait_for_status "$member_socket" \
  ".knocks | length == 1 and .[0].id == \"$first_id\" and .[0].from.display_name == \"Owner\""

duplicate_knock=$(target/debug/wispctl --socket "$owner_socket" join MemberA)
duplicate_id=$(jq -r '.knock_id' <<<"$duplicate_knock")
[[ "$duplicate_id" == "$first_id" ]]
wait_for_status "$member_socket" '.knocks | length == 1'

target/debug/wispctl --socket "$member_socket" knock "$first_id" later \
  | jq -e '.status == "later"' >/dev/null
wait_for_status "$member_socket" '.knocks == [] and .self.hangout_id == null'
wait_for_status "$owner_socket" '.self.hangout_id == null'

expiring_knock=$(target/debug/wispctl --socket "$owner_socket" join MemberA)
expiring_id=$(jq -r '.knock_id' <<<"$expiring_knock")
wait_for_status "$member_socket" \
  ".knocks | length == 1 and .[0].id == \"$expiring_id\""
sleep 3.2
wait_for_status "$member_socket" '.knocks == []'
if target/debug/wispctl --socket "$member_socket" knock "$expiring_id" join \
  >"$test_dir/expected-expired.log" 2>&1; then
  echo "Expired knock was unexpectedly accepted" >&2
  exit 1
fi
rg -q 'knock_unavailable' "$test_dir/expected-expired.log"

requester_offline_knock=$(target/debug/wispctl --socket "$owner_socket" join MemberA)
requester_offline_id=$(jq -r '.knock_id' <<<"$requester_offline_knock")
wait_for_status "$member_socket" \
  ".knocks | length == 1 and .[0].id == \"$requester_offline_id\""
stop_process INT "$owner_pid"
wait "$owner_pid" 2>/dev/null || true
owner_pid=""
wait_for_status "$member_socket" '
  any(.friends[]; .display_name == "Owner" and .online == false)
'
if target/debug/wispctl --socket "$member_socket" knock "$requester_offline_id" join \
  >"$test_dir/expected-requester-offline.log" 2>&1; then
  echo "Knock from an offline requester was unexpectedly accepted" >&2
  exit 1
fi
rg -q 'knock_requester_offline' "$test_dir/expected-requester-offline.log"
wait_for_status "$member_socket" '.knocks == []'
start_owner
for _ in $(seq 1 100); do
  [[ -S "$owner_socket" ]] && break
  sleep 0.05
done
[[ -S "$owner_socket" ]]
wait_for_status "$member_socket" '
  any(.friends[]; .display_name == "Owner" and .online == true)
'

accepted_knock=$(target/debug/wispctl --socket "$owner_socket" join MemberA)
accepted_id=$(jq -r '.knock_id' <<<"$accepted_knock")
wait_for_status "$member_socket" \
  ".knocks | length == 1 and .[0].id == \"$accepted_id\""
accepted_result=$(target/debug/wispctl --socket "$member_socket" knock "$accepted_id" join)
hangout_id=$(jq -r 'select(.status == "accepted") | .hangout_id' <<<"$accepted_result")
[[ -n "$hangout_id" && "$hangout_id" != "null" ]]
wait_for_status "$member_socket" \
  ".self.connection == \"connected\" and .self.hangout_id == \"$hangout_id\" and .knocks == []"
wait_for_status "$owner_socket" \
  ".self.connection == \"connected\" and .self.hangout_id == \"$hangout_id\""

target/debug/wispctl --socket "$owner_socket" leave
target/debug/wispctl --socket "$member_socket" leave
stop_process INT "$member_pid"
wait "$member_pid" 2>/dev/null || true
member_pid=""
wait_for_status "$owner_socket" '
  any(.friends[]; .display_name == "MemberA" and .online == false)
'
if target/debug/wispctl --socket "$owner_socket" join MemberA \
  >"$test_dir/expected-offline.log" 2>&1; then
  echo "Knock to an offline friend unexpectedly succeeded" >&2
  exit 1
fi
rg -q 'friend_offline' "$test_dir/expected-offline.log"

WISP_SERVER_URL="$server_url" RUST_LOG=info \
  target/debug/wisp-sim --profile MemberA --presence knock --auto-respond-knocks join \
  >"$test_dir/sim.log" 2>&1 &
sim_pid=$!
wait_for_status "$owner_socket" '
  any(.friends[]; .display_name == "MemberA" and .online == true and .presence == "knock")
'
sim_knock=$(target/debug/wispctl --socket "$owner_socket" join MemberA)
jq -e '.status == "knock_sent"' <<<"$sim_knock" >/dev/null
wait_for_status "$owner_socket" '
  .self.connection == "connected" and .self.hangout_id != null
'
rg -q 'automatically responded to knock' "$test_dir/sim.log"

echo "Knock integration test passed (deduplicate, Later, expiry, Join, requester/recipient offline, simulator auto-response)."
