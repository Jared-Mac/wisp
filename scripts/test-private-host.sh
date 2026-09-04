#!/usr/bin/env bash
set -euo pipefail
repo_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
cd "$repo_dir"
cargo build -p wisp-server --locked
test_dir=$(mktemp -d /tmp/wisp-private-host.XXXXXX)
server_pid=""
cleanup() {
  trap - EXIT INT TERM
  if [[ -n "$server_pid" ]]; then kill "$server_pid" 2>/dev/null || true; wait "$server_pid" 2>/dev/null || true; fi
  [[ "$test_dir" == /tmp/wisp-private-host.* && -d "$test_dir" ]] && rm -rf -- "$test_dir"
}
trap cleanup EXIT INT TERM
port=$(shuf -i 22000-45000 -n 1)
common=(--require-chat-e2ee --database-url "sqlite://$test_dir/fresh.sqlite3" --addr "127.0.0.1:$port")
secure=(--allow-dev-sessions false --livekit-url wss://isolated.invalid --livekit-api-key isolated-test --livekit-api-secret isolated-test-service-secret-not-a-real-credential)
if timeout 5 env -u WISP_E2EE_KEY -u WISP_BOOTSTRAP_TOKEN target/debug/wisp-server "${common[@]}" --allow-dev-sessions true >"$test_dir/refused.log" 2>&1; then
  echo 'Private server accepted development authentication' >&2; exit 1
fi
rg -q 'requires WISP_ALLOW_DEV_SESSIONS=false' "$test_dir/refused.log"
if timeout 5 env -u WISP_E2EE_KEY -u WISP_BOOTSTRAP_TOKEN target/debug/wisp-server "${common[@]}" --allow-dev-sessions false --livekit-url wss://isolated.invalid >"$test_dir/refused.log" 2>&1; then
  echo 'Private server accepted built-in media service credentials' >&2; exit 1
fi
rg -q 'requires fresh LiveKit service credentials' "$test_dir/refused.log"
if WISP_E2EE_KEY=isolated-test-client-key timeout 5 env -u WISP_BOOTSTRAP_TOKEN target/debug/wisp-server "${common[@]}" "${secure[@]}" >"$test_dir/refused.log" 2>&1; then
  echo 'Private server accepted a client media key' >&2; exit 1
fi
rg -q 'must never be installed on the server' "$test_dir/refused.log"
env -u WISP_E2EE_KEY -u WISP_BOOTSTRAP_TOKEN target/debug/wisp-server "${common[@]}" "${secure[@]}" >"$test_dir/server.log" 2>&1 &
server_pid=$!
ready=false
for _ in $(seq 1 100); do
  if curl --silent --fail "http://127.0.0.1:$port/healthz" | jq -e '.ok == true and .database == true' >/dev/null; then ready=true; break; fi
  kill -0 "$server_pid" 2>/dev/null || { echo 'Isolated private host exited' >&2; exit 1; }
  sleep 0.05
done
[[ "$ready" == true ]] || { echo 'Private host health check timed out' >&2; exit 1; }
[[ $(curl --silent --output /dev/null --write-out '%{http_code}' -H 'content-type: application/json' --data '{"profile":"Jared"}' "http://127.0.0.1:$port/v1/dev/session") != 200 ]]
[[ $(sqlite3 "$test_dir/fresh.sqlite3" 'SELECT COUNT(*) FROM messages') == 0 ]]
echo 'Fresh strict host starts; development login, default service credentials, and server-held media keys are rejected'
