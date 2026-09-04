#!/usr/bin/env bash
# Source-only endpoint parser. HTTPS origins are used for hosted installations;
# bare hostnames retain the private-alpha Tailscale transport for compatibility.
wisp_resolve_endpoint() {
  local endpoint=${1:-}
  if [[ "$endpoint" =~ ^https://([A-Za-z0-9][A-Za-z0-9.-]*)(:([0-9]{1,5}))?/?$ ]]; then
    if [[ -n ${BASH_REMATCH[3]} ]] && (( 10#${BASH_REMATCH[3]} < 1 || 10#${BASH_REMATCH[3]} > 65535 )); then
      echo "Invalid HTTPS port" >&2; return 2
    fi
    WISP_SELECTED_SERVER_HOST=${BASH_REMATCH[1]}
    WISP_SELECTED_SERVER_URL=${endpoint%/}
  elif [[ "$endpoint" =~ ^[A-Za-z0-9][A-Za-z0-9.-]*$ ]]; then
    WISP_SELECTED_SERVER_HOST=$endpoint
    WISP_SELECTED_SERVER_URL="http://$endpoint:8787"
  else
    echo "Use an HTTPS origin (https://wisp.example.com) or a private-alpha Tailscale host; no credentials, paths, or query strings" >&2
    return 2
  fi
}
