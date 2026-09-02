#!/usr/bin/env bash
set -euo pipefail

action=${1:-open}

detect_primary_screen() {
  if [[ -n ${WISP_PRIMARY_SCREEN:-} ]]; then
    printf '%s\n' "$WISP_PRIMARY_SCREEN"
    return
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

case "$action" in
  open|show|close|hide|toggle|quit)
    if qs "${selector[@]}" ipc call dev.wisp "$action"; then
      exit 0
    fi

    case "$action" in
      open|show|toggle)
        qs "${selector[@]}" --daemonize
        for _ in $(seq 1 40); do
          if qs "${selector[@]}" ipc call dev.wisp open 2>/dev/null; then
            exit 0
          fi
          sleep 0.05
        done
        echo "Wisp UI started but its IPC endpoint did not become ready" >&2
        exit 1
        ;;
      *)
        # Closing an application that is not running is already complete.
        exit 0
        ;;
    esac
    ;;
  activate)
    x=${2:-0}
    y=${3:-0}
    if qs "${selector[@]}" ipc call dev.wisp activate "$x" "$y"; then
      exit 0
    fi
    qs "${selector[@]}" --daemonize
    for _ in $(seq 1 40); do
      if qs "${selector[@]}" ipc call dev.wisp activate "$x" "$y" 2>/dev/null; then
        exit 0
      fi
      sleep 0.05
    done
    echo "Wisp UI started but its IPC endpoint did not become ready" >&2
    exit 1
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
    if qs "${selector[@]}" ipc call dev.wisp anchor "$position"; then
      exit 0
    fi
    qs "${selector[@]}" --daemonize
    for _ in $(seq 1 40); do
      if qs "${selector[@]}" ipc call dev.wisp anchor "$position" 2>/dev/null; then
        exit 0
      fi
      sleep 0.05
    done
    echo "Wisp UI started but its IPC endpoint did not become ready" >&2
    exit 1
    ;;
  status)
    exec qs "${selector[@]}" ipc call dev.wisp.bridge status
    ;;
  desktop)
    exec qs "${selector[@]}" ipc call dev.wisp desktop
    ;;
  *)
    echo "usage: wisp-ui {open|show|close|hide|toggle|quit|activate X Y|anchor POSITION|status|desktop}" >&2
    exit 2
    ;;
esac
