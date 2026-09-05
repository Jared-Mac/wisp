#!/usr/bin/env bash
set -euo pipefail
case "$(basename "$0")" in
  wisp-client)
    printf 'client\n' >>"$WISP_TEST_STATE/calls"
    [[ -f "$WISP_TEST_STATE/authenticated" ]] && exit 0
    exit 2
    ;;
  wisp-onboarding)
    printf 'prompt:%s\n' "$WISP_ONBOARDING_MODE" >>"$WISP_TEST_STATE/calls"
    if [[ "$WISP_TEST_OUTCOME" == cancel ]]; then exit 2; fi
    touch "$WISP_TEST_STATE/authenticated"
    ;;
esac
