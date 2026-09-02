#!/usr/bin/env bash
set -euo pipefail

repo_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)

if ! command -v qs >/dev/null 2>&1; then
  echo "Skipping standalone UI test: Quickshell is not installed"
  exit 0
fi

test_root=$(mktemp -d)
config_dir="$test_root/wisp-ui-test"
log_file="$test_root/quickshell.log"
mkdir -p "$config_dir"
cp -a "$repo_dir/quickshell/app/". "$config_dir/"

ui_pid=""
cleanup() {
  trap - EXIT INT TERM
  qs --path "$config_dir" kill >/dev/null 2>&1 || true
  if [[ -n "$ui_pid" ]]; then
    wait "$ui_pid" 2>/dev/null || true
  fi
  rm -rf -- "$test_root"
}
trap cleanup EXIT INT TERM

qs --path "$config_dir" --no-duplicate >"$log_file" 2>&1 &
ui_pid=$!

for _ in $(seq 1 80); do
  if qs --path "$config_dir" ipc show 2>/dev/null | grep -q "target dev.wisp"; then
    break
  fi
  if ! kill -0 "$ui_pid" 2>/dev/null; then
    cat "$log_file" >&2
    exit 1
  fi
  sleep 0.05
done

status=$(qs --path "$config_dir" ipc call dev.wisp.bridge status)
printf '%s' "$status" | jq -e '
  (.connected | type == "boolean") and
  (.socket | endswith("/wisp/wispd.sock")) and
  (.snapshot | type == "object")
' >/dev/null

qs --path "$config_dir" ipc call dev.wisp hide
qs --path "$config_dir" ipc call dev.wisp open
qs --path "$config_dir" ipc call dev.wisp quit
wait "$ui_pid"
ui_pid=""

if grep -Eq "Failed to load configuration|QQmlApplicationEngine failed|ReferenceError|TypeError" "$log_file"; then
  cat "$log_file" >&2
  exit 1
fi

echo "Standalone Quickshell UI loaded and passed IPC lifecycle checks"
