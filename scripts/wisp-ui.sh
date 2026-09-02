#!/usr/bin/env bash
set -euo pipefail

action=${1:-open}

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
        exec qs "${selector[@]}" --daemonize
        ;;
      *)
        # Closing an application that is not running is already complete.
        exit 0
        ;;
    esac
    ;;
  status)
    exec qs "${selector[@]}" ipc call dev.wisp.bridge status
    ;;
  *)
    echo "usage: wisp-ui {open|show|close|hide|toggle|quit|status}" >&2
    exit 2
    ;;
esac
