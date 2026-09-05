#!/usr/bin/env bash
set -euo pipefail
case "$(basename "$0")" in
  systemctl)
    [[ ${WISP_TEST_SYSTEMD:-yes} == yes ]] || exit 1
    case "$2" in
      show-environment) exit 0 ;;
      is-active) [[ -f "$WISP_TEST_STATE/active" ]] ;;
      start)
        if [[ ! -f "$WISP_TEST_STATE/active" ]]; then
          "$(dirname "$0")/wisp-launch"
        fi
        ;;
      *) exit 1 ;;
    esac
    ;;
  pgrep) [[ -f "$WISP_TEST_STATE/active" ]] ;;
  wisp-ui) printf 'ui:%s\n' "$*" >>"$WISP_TEST_STATE/calls" ;;
  wisp)
    [[ ${WISP_INTEGRATION:-} == omarchy ]]
    printf 'wisp:%s\n' "$*" >>"$WISP_TEST_STATE/calls"
    ;;
esac
