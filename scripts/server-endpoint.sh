#!/usr/bin/env bash
# Source-only endpoint parser. Public servers require HTTPS; plain HTTP is
# accepted only for development on the same machine.
wisp_resolve_endpoint() {
  local endpoint=${1:-}
  if [[ "$endpoint" =~ ^https://([A-Za-z0-9][A-Za-z0-9.-]*)(:([0-9]{1,5}))?/?$ ]]; then
    if [[ -n ${BASH_REMATCH[3]} ]] && (( 10#${BASH_REMATCH[3]} < 1 || 10#${BASH_REMATCH[3]} > 65535 )); then
      echo "Invalid HTTPS port" >&2; return 2
    fi
    WISP_SELECTED_SERVER_HOST=${BASH_REMATCH[1]}
    WISP_SELECTED_SERVER_URL=${endpoint%/}
  elif [[ "$endpoint" =~ ^http://(localhost|127\.0\.0\.1|\[::1\])(:([0-9]{1,5}))?/?$ ]]; then
    WISP_SELECTED_SERVER_HOST=${BASH_REMATCH[1]}
    WISP_SELECTED_SERVER_URL=${endpoint%/}
  else
    echo "Use an HTTPS origin such as https://wisp.example.com; plain HTTP is limited to localhost development" >&2
    return 2
  fi
}
