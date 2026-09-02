#!/usr/bin/env bash
set -euo pipefail

detect_primary_screen() {
  if [[ -n ${WISP_PRIMARY_SCREEN:-} ]]; then
    printf '%s\n' "$WISP_PRIMARY_SCREEN"
    return
  fi
  if command -v hyprctl >/dev/null 2>&1 && command -v jq >/dev/null 2>&1; then
    local screen
    screen=$(hyprctl monitors -j 2>/dev/null \
      | jq -r '([.[] | select(.focused == true)][0].name // .[0].name) // empty' \
      2>/dev/null || true)
    if [[ -n "$screen" ]]; then
      printf '%s\n' "$screen"
      return
    fi
  fi
  if command -v kscreen-doctor >/dev/null 2>&1 && command -v jq >/dev/null 2>&1; then
    local screen
    screen=$(kscreen-doctor -j 2>/dev/null | jq -r '
      ([.outputs[] | select(.enabled == true and .connected == true and (.priority // 0) > 0)]
        | min_by(.priority) | .name) // empty
    ' 2>/dev/null || true)
    if [[ -n "$screen" ]]; then
      printf '%s\n' "$screen"
      return
    fi
  fi
  if command -v xrandr >/dev/null 2>&1; then
    xrandr --query 2>/dev/null | awk '/ connected primary / { print $1; exit }'
  fi
}

primary_screen=$(detect_primary_screen)
if [[ -n "$primary_screen" ]]; then
  export WISP_PRIMARY_SCREEN="$primary_screen"
fi

if ! command -v qs >/dev/null 2>&1; then
  echo "Quickshell is required to run the Wisp UI" >&2
  exit 1
fi

if [[ -n ${WISP_QUICKSHELL_PATH:-} ]]; then
  selector=(--path "$WISP_QUICKSHELL_PATH")
else
  selector=(--config "${WISP_QUICKSHELL_CONFIG:-wisp}")
fi

start_ui() {
  qs "${selector[@]}" --daemonize >/dev/null 2>&1
}

call_ui() {
  qs "${selector[@]}" ipc call "$@"
}

call_or_start() {
  local endpoint=$1
  local action=$2
  shift 2

  if call_ui "$endpoint" "$action" "$@" >/dev/null 2>&1; then
    return
  fi

  case "$action" in
    open|show|toggle|activate|anchor)
      start_ui
      for _ in $(seq 1 40); do
        if call_ui "$endpoint" "$action" "$@" 2>/dev/null; then
          return
        fi
        sleep 0.05
      done
      echo "Wisp UI started but its IPC endpoint did not become ready" >&2
      exit 1
      ;;
    close|hide)
      # Hiding a surface whose UI process is not running is already complete.
      return
      ;;
  esac
}

surface=${1:-app}
case "$surface" in
  app|panel)
    action=${2:-open}
    shift $(( $# > 0 ? 1 : 0 ))
    shift $(( $# > 0 ? 1 : 0 ))
    endpoint="dev.wisp.$surface"
    case "$action" in
      open|show|close|hide|toggle)
        call_or_start "$endpoint" "$action"
        ;;
      desktop)
        call_ui "$endpoint" desktop
        ;;
      activate)
        if [[ "$surface" != "panel" ]]; then
          echo "activate is only available for the panel surface" >&2
          exit 2
        fi
        call_or_start "$endpoint" activate "${1:-0}" "${2:-0}"
        ;;
      anchor)
        if [[ "$surface" != "panel" ]]; then
          echo "anchor is only available for the panel surface" >&2
          exit 2
        fi
        position=${1:-}
        case "$position" in
          auto|bottom-right|bottom-left|top-right|top-left) ;;
          *)
            echo "anchor must be auto, bottom-right, bottom-left, top-right, or top-left" >&2
            exit 2
            ;;
        esac
        call_or_start "$endpoint" anchor "$position"
        ;;
      *)
        echo "usage: wisp-ui {app|panel} {open|show|close|hide|toggle|desktop}" >&2
        exit 2
        ;;
    esac
    ;;
  status)
    call_ui dev.wisp.bridge status
    ;;
  quit)
    call_ui dev.wisp quit >/dev/null 2>&1 || true
    ;;
  open|show|close|hide|toggle)
    # Compatibility: the historical standalone commands now address the app.
    call_or_start dev.wisp.app "$surface"
    ;;
  activate)
    call_or_start dev.wisp.panel activate "${2:-0}" "${3:-0}"
    ;;
  anchor)
    position=${2:-}
    case "$position" in
      auto|bottom-right|bottom-left|top-right|top-left) ;;
      *)
        echo "anchor must be auto, bottom-right, bottom-left, top-right, or top-left" >&2
        exit 2
        ;;
    esac
    call_or_start dev.wisp.panel anchor "$position"
    ;;
  desktop)
    call_ui dev.wisp.app desktop
    ;;
  *)
    echo "usage: wisp-ui {app|panel} ACTION | {open|toggle|status|quit}" >&2
    exit 2
    ;;
esac
