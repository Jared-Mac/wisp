#!/usr/bin/env bash
set -euo pipefail
case "$(basename "$0")" in
  omarchy)
    case "$*" in
      'plugin validate '*) exit 0 ;;
      'plugin list --json') printf '%s\n' '[{"id":"dev.wisp","enabled":false}]' ;;
      *) echo 'Unexpected Omarchy mutation in update test' >&2; exit 1 ;;
    esac
    ;;
  omarchy-shell)
    [[ "$*" == 'shell rescanPlugins' ]]
    touch "$WISP_TEST_STATE/rescanned"
    ;;
  app-sync.sh) touch "$WISP_TEST_STATE/app-synced" ;;
esac
