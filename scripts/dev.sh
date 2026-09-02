#!/usr/bin/env bash
set -euo pipefail

repo_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
cd "$repo_dir"

child_pids=()
compose_started=0

cleanup() {
  trap - EXIT INT TERM
  for pid in "${child_pids[@]}"; do
    kill "$pid" 2>/dev/null || true
  done
  for pid in "${child_pids[@]}"; do
    wait "$pid" 2>/dev/null || true
  done
  if [[ $compose_started -eq 1 ]]; then
    docker compose -f infra/local/compose.yaml down >/dev/null 2>&1 || true
  fi
}
trap cleanup EXIT INT TERM

livekit_config=${WISP_LIVEKIT_CONFIG:-infra/local/livekit.yaml}

if [[ -x "$repo_dir/.tools/livekit/livekit-server" ]]; then
  "$repo_dir/.tools/livekit/livekit-server" --config "$livekit_config" &
  child_pids+=("$!")
elif command -v livekit-server >/dev/null 2>&1; then
  livekit-server --config "$livekit_config" &
  child_pids+=("$!")
elif [[ -n ${WISP_LIVEKIT_CONFIG:-} ]]; then
  echo "A custom LiveKit config requires the local binary; run 'just livekit-install'" >&2
  exit 1
elif command -v docker >/dev/null 2>&1 && docker info >/dev/null 2>&1; then
  docker compose -f infra/local/compose.yaml up -d livekit
  compose_started=1
else
  echo "LiveKit is not installed; run 'just livekit-install' first" >&2
  exit 1
fi

cargo run -p wisp-server &
child_pids+=("$!")

for _ in $(seq 1 80); do
  if curl --silent --fail "${WISP_SERVER_URL:-http://127.0.0.1:8787}/healthz" >/dev/null; then break; fi
  sleep 0.1
done

cargo run -p wispd -- --profile "${WISP_PROFILE:-Jared}" &
child_pids+=("$!")

echo "Wisp is running. Use 'just sim Tyler' or 'cargo run -p wispctl -- status'."
wait -n "${child_pids[@]}"
