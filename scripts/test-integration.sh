#!/usr/bin/env bash
set -euo pipefail

repo_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
cd "$repo_dir"

test_dir=$(mktemp -d)
server_pid=""
daemon_pid=""
sim_pid=""

cleanup() {
  trap - EXIT INT TERM
  for pid in "$sim_pid" "$daemon_pid" "$server_pid"; do
    [[ -z "$pid" ]] || kill "$pid" 2>/dev/null || true
  done
  for pid in "$sim_pid" "$daemon_pid" "$server_pid"; do
    [[ -z "$pid" ]] || wait "$pid" 2>/dev/null || true
  done
  rm -rf -- "$test_dir"
}
trap cleanup EXIT INT TERM

port=$(shuf -i 20000-45000 -n 1)
export WISP_SERVER_URL="http://127.0.0.1:$port"
export WISP_SERVER_ADDR="127.0.0.1:$port"
export WISP_DATABASE_URL="sqlite://$test_dir/wisp.sqlite3"
export XDG_RUNTIME_DIR="$test_dir/runtime"
export WISP_HYPR_CONFIG_DIR="$test_dir/hypr"
export WISP_WISPCTL_PATH="$repo_dir/target/debug/wispctl"
mkdir -p "$XDG_RUNTIME_DIR"
mkdir -p "$WISP_HYPR_CONFIG_DIR"
printf '%s\n' '-- integration test bindings' >"$WISP_HYPR_CONFIG_DIR/bindings.lua"

cargo build --workspace
target/debug/wisp-server >"$test_dir/server.log" 2>&1 &
server_pid=$!

for _ in $(seq 1 100); do
  if curl --silent --fail "$WISP_SERVER_URL/healthz" >/dev/null; then break; fi
  sleep 0.05
done
curl --silent --fail "$WISP_SERVER_URL/healthz" >/dev/null

target/debug/wisp-sim --profile Tyler --silent >"$test_dir/sim.log" 2>&1 &
sim_pid=$!
target/debug/wispd --profile Jared --disable-media >"$test_dir/daemon.log" 2>&1 &
daemon_pid=$!

for _ in $(seq 1 100); do
  [[ -S "$XDG_RUNTIME_DIR/wisp/wispd.sock" ]] && break
  sleep 0.05
done
[[ -S "$XDG_RUNTIME_DIR/wisp/wispd.sock" ]]

target/debug/wispctl status | rg '"display_name": "Jared"' >/dev/null
target/debug/wispctl ptt shortcut F8 | jq -e '.shortcut == "F8"' >/dev/null
target/debug/wispctl status | jq -e '.self.push_to_talk.shortcut == "F8"' >/dev/null
rg -F 'require("hypr.wisp")' "$WISP_HYPR_CONFIG_DIR/bindings.lua" >/dev/null
rg -F 'hl.unbind("F8")' "$WISP_HYPR_CONFIG_DIR/wisp.lua" >/dev/null
rg -F 'release = true' "$WISP_HYPR_CONFIG_DIR/wisp.lua" >/dev/null
target/debug/wispctl ptt clear-shortcut | jq -e '.shortcut == null' >/dev/null
target/debug/wispctl status | jq -e '.self.push_to_talk.shortcut == null' >/dev/null
rg -F 'No shortcut is currently set.' "$WISP_HYPR_CONFIG_DIR/wisp.lua" >/dev/null
target/debug/wispctl presence knock | rg '"presence": "knock"' >/dev/null
target/debug/wispctl join Tyler
target/debug/wispctl status | rg '"connection": "connected"' >/dev/null
target/debug/wispctl mute | rg '"muted": true' >/dev/null
target/debug/wispctl leave
target/debug/wispctl status | rg '"connection": "available"' >/dev/null

echo "Integration smoke test passed."
