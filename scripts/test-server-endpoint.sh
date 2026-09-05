#!/usr/bin/env bash
set -euo pipefail
source "$(dirname -- "${BASH_SOURCE[0]}")/server-endpoint.sh"
wisp_resolve_endpoint 'https://wisp.example.com/'
[[ "$WISP_SELECTED_SERVER_URL" == 'https://wisp.example.com' && "$WISP_SELECTED_SERVER_HOST" == 'wisp.example.com' ]]
wisp_resolve_endpoint 'https://wisp.example.com:8443'
[[ "$WISP_SELECTED_SERVER_URL" == 'https://wisp.example.com:8443' ]]
wisp_resolve_endpoint 'http://127.0.0.1:8787'
[[ "$WISP_SELECTED_SERVER_URL" == 'http://127.0.0.1:8787' ]]
for invalid in 'private-host.tailnet.ts.net' 'http://public.example.com' 'https://user:password@example.com' 'https://example.com/path' 'https://example.com?key=secret' 'https://example.com:0' 'https://example.com:65536' 'https://example.com#fragment'; do
  if wisp_resolve_endpoint "$invalid" 2>/dev/null; then echo 'Invalid endpoint accepted' >&2; exit 1; fi
done
echo 'HTTPS origins, local development, and credential/query rejection passed'
