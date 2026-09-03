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
primary_screen=""
if command -v hyprctl >/dev/null 2>&1 && command -v jq >/dev/null 2>&1; then
  primary_screen=$(hyprctl monitors -j 2>/dev/null \
    | jq -r '([.[] | select(.focused == true)][0].name // .[0].name) // empty' \
    2>/dev/null || true)
fi
if [[ -z "$primary_screen" ]] \
  && command -v kscreen-doctor >/dev/null 2>&1 \
  && command -v jq >/dev/null 2>&1; then
  primary_screen=$(kscreen-doctor -j 2>/dev/null | jq -r '
    ([.outputs[] | select(.enabled == true and .connected == true and (.priority // 0) > 0)]
      | min_by(.priority) | .name) // empty
  ' 2>/dev/null || true)
fi
if [[ -z "$primary_screen" ]] && command -v xrandr >/dev/null 2>&1; then
  primary_screen=$(xrandr --query 2>/dev/null | awk '/ connected primary / { print $1; exit }')
fi
cleanup() {
  trap - EXIT INT TERM
  qs --path "$config_dir" kill >/dev/null 2>&1 || true
  if [[ -n "$ui_pid" ]]; then
    wait "$ui_pid" 2>/dev/null || true
  fi
  rm -rf -- "$test_root"
}
trap cleanup EXIT INT TERM

WISP_PRIMARY_SCREEN="$primary_screen" \
  qs --path "$config_dir" --no-duplicate >"$log_file" 2>&1 &
ui_pid=$!

for _ in $(seq 1 80); do
  if qs --path "$config_dir" ipc show 2>/dev/null \
    | grep -q "target dev.wisp.app"; then
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

qs --path "$config_dir" ipc call dev.wisp.panel hide
qs --path "$config_dir" ipc call dev.wisp.panel open
panel=$(qs --path "$config_dir" ipc call dev.wisp.panel desktop)
printf '%s' "$panel" | jq -e '
  .visible == true and
  .width >= 420 and .height >= 760 and
  (.resolved_anchor | IN("bottom-right", "bottom-left", "top-right", "top-left")) and
  (.screen | type == "string")
' >/dev/null
if [[ -n "$primary_screen" ]]; then
  printf '%s' "$panel" | jq -e --arg primary "$primary_screen" \
    '.screen == $primary' >/dev/null
fi

qs --path "$config_dir" ipc call dev.wisp.panel anchor top-left
panel=$(qs --path "$config_dir" ipc call dev.wisp.panel desktop)
printf '%s' "$panel" | jq -e '
  .visible == true and .anchor == "top-left" and .resolved_anchor == "top-left"
' >/dev/null

qs --path "$config_dir" ipc call dev.wisp.app open
app=$(qs --path "$config_dir" ipc call dev.wisp.app desktop)
panel=$(qs --path "$config_dir" ipc call dev.wisp.panel desktop)
printf '%s' "$app" | jq -e '
  .visible == true and .width >= 420 and .height >= 800 and
  (.wide_layout | type == "boolean")
' >/dev/null
printf '%s' "$panel" | jq -e '.visible == false' >/dev/null

qs --path "$config_dir" ipc call dev.wisp quit
wait "$ui_pid"
ui_pid=""

if grep -Eq "Failed to load configuration|QQmlApplicationEngine failed|ReferenceError|TypeError" "$log_file"; then
  cat "$log_file" >&2
  exit 1
fi

echo "Standalone Quickshell app and compact panel passed IPC lifecycle checks"
