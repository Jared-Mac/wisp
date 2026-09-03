#!/usr/bin/env bash
set -euo pipefail

repo_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
cd "$repo_dir"

test_dir=$(mktemp -d)
server_pid=""
writer_pid=""
cleanup() {
  trap - EXIT INT TERM
  [[ -z "$writer_pid" ]] || kill "$writer_pid" 2>/dev/null || true
  [[ -z "$writer_pid" ]] || wait "$writer_pid" 2>/dev/null || true
  [[ -z "$server_pid" ]] || kill "$server_pid" 2>/dev/null || true
  [[ -z "$server_pid" ]] || wait "$server_pid" 2>/dev/null || true
  if [[ ${WISP_KEEP_TEST_LOGS:-0} == 1 ]]; then
    echo "Private-alpha logs preserved at $test_dir"
  else
    rm -rf -- "$test_dir"
  fi
}
trap cleanup EXIT INT TERM

for command_name in curl jq sqlite3; do
  command -v "$command_name" >/dev/null 2>&1 || {
    echo "$command_name is required for the private-alpha test" >&2
    exit 1
  }
done

cargo build -p wisp-server
port=$(shuf -i 20000-45000 -n 1)
server_url="http://127.0.0.1:$port"
database="$test_dir/wisp.sqlite3"
bootstrap_token="private-alpha-bootstrap-test-token"

start_server() {
  WISP_SERVER_ADDR="127.0.0.1:$port" \
  WISP_DATABASE_URL="sqlite://$database" \
  WISP_ALLOW_DEV_SESSIONS=false \
  WISP_BOOTSTRAP_TOKEN="$bootstrap_token" \
    target/debug/wisp-server >>"$test_dir/server.log" 2>&1 &
  server_pid=$!
  for _ in $(seq 1 100); do
    if curl --silent --fail "$server_url/healthz" >/dev/null; then return; fi
    sleep 0.05
  done
  echo "private-alpha server did not become healthy" >&2
  exit 1
}

stop_server() {
  kill "$server_pid"
  wait "$server_pid"
  server_pid=""
}

post_json() {
  local path=$1
  local body=$2
  local token=${3:-}
  local auth=()
  [[ -z "$token" ]] || auth=(-H "authorization: Bearer $token")
  curl --silent --show-error --fail-with-body \
    -H 'content-type: application/json' "${auth[@]}" \
    --data-binary "$body" "$server_url$path"
}

new_session() {
  local device_id=$1
  local device_token=$2
  post_json /v1/sessions "$(jq -cn \
    --arg id "$device_id" --arg token "$device_token" \
    '{device_id:$id,device_token:$token,protocol_version:1}')"
}

start_server
curl --silent --fail "$server_url/healthz" \
  | jq -e '.ok == true and .database == true and .protocol_version == 1' >/dev/null

if post_json /v1/dev/session '{"profile":"Jared"}' >/dev/null 2>&1; then
  echo "development login was available on a private-alpha server" >&2
  exit 1
fi

jared_credential=$(post_json /v1/devices/bootstrap "$(jq -cn \
  --arg token "$bootstrap_token" \
  '{bootstrap_token:$token,profile:"Jared",device_name:"Host",protocol_version:1}')")
jared_device_id=$(jq -er '.device_id' <<<"$jared_credential")
jared_device_token=$(jq -er '.device_token' <<<"$jared_credential")
if post_json /v1/devices/bootstrap "$(jq -cn \
  --arg token "$bootstrap_token" \
  '{bootstrap_token:$token,profile:"Jared",device_name:"Second host",protocol_version:1}')" \
  >/dev/null 2>&1; then
  echo "administrator bootstrap was accepted twice" >&2
  exit 1
fi
jared_session=$(new_session "$jared_device_id" "$jared_device_token")
jared_token=$(jq -er '.token' <<<"$jared_session")
jq -e '.protocol_version == 1 and .user.display_name == "Jared"' <<<"$jared_session" >/dev/null

invite=$(post_json /v1/admin/invites \
  '{"profile":"Tyler","expires_in_minutes":5}' "$jared_token")
invite_code=$(jq -er '.code' <<<"$invite")
tyler_credential=$(post_json /v1/devices/register "$(jq -cn \
  --arg invite "$invite_code" \
  '{invite_code:$invite,device_name:"Tyler laptop",protocol_version:1}')")
tyler_device_id=$(jq -er '.device_id' <<<"$tyler_credential")
tyler_device_token=$(jq -er '.device_token' <<<"$tyler_credential")
tyler_session=$(new_session "$tyler_device_id" "$tyler_device_token")
tyler_token=$(jq -er '.token' <<<"$tyler_session")

if [[ $(curl --silent --output /dev/null --write-out '%{http_code}' --http1.1 \
  -H 'connection: Upgrade' -H 'upgrade: websocket' \
  -H 'sec-websocket-version: 13' -H 'sec-websocket-key: d2lzcC10ZXN0LWtleQ==' \
  "$server_url/v1/events?token=$tyler_token") != 401 ]]; then
  echo "a device session was accepted in a query string" >&2
  exit 1
fi
if [[ $(curl --silent --output /dev/null --write-out '%{http_code}' \
  -H "authorization: $tyler_token" "$server_url/v1/snapshot") != 401 ]]; then
  echo "a non-Bearer authorization header was accepted" >&2
  exit 1
fi

if post_json /v1/devices/register "$(jq -cn \
  --arg invite "$invite_code" \
  '{invite_code:$invite,device_name:"Replay",protocol_version:1}')" >/dev/null 2>&1; then
  echo "one-use invite was accepted twice" >&2
  exit 1
fi

conversation=$(post_json /v1/conversations/direct \
  '{"friend":"Tyler"}' "$jared_token")
conversation_id=$(jq -er '.id' <<<"$conversation")
post_json /v1/messages "$(jq -cn --arg id "$conversation_id" \
  '{conversation_id:$id,content_type:"text/plain",payload:"offline hello",encryption_version:0}')" \
  "$jared_token" >/dev/null

curl --silent --fail -H "authorization: Bearer $tyler_token" \
  --get --data-urlencode "conversation_id=$conversation_id" "$server_url/v1/messages" \
  | jq -e 'length == 1 and .[0].payload == "offline hello"' >/dev/null
curl --silent --fail -H "authorization: Bearer $tyler_token" "$server_url/v1/snapshot" \
  | jq -e --arg id "$conversation_id" \
      '.conversations[] | select(.id == $id) | .unread_count == 1' >/dev/null
post_json /v1/conversations/read "$(jq -cn --arg id "$conversation_id" \
  '{conversation_id:$id}')" "$tyler_token" >/dev/null

charlie_invite=$(post_json /v1/admin/invites \
  '{"profile":"Charlie","expires_in_minutes":5}' "$jared_token")
charlie_credential=$(post_json /v1/devices/register "$(jq -cn \
  --arg invite "$(jq -er '.code' <<<"$charlie_invite")" \
  '{invite_code:$invite,device_name:"Charlie laptop",protocol_version:1}')")
charlie_session=$(new_session \
  "$(jq -er '.device_id' <<<"$charlie_credential")" \
  "$(jq -er '.device_token' <<<"$charlie_credential")")
charlie_token=$(jq -er '.token' <<<"$charlie_session")
if curl --silent --fail -H "authorization: Bearer $charlie_token" \
  --get --data-urlencode "conversation_id=$conversation_id" "$server_url/v1/messages" \
  >/dev/null 2>&1; then
  echo "non-member read a private conversation" >&2
  exit 1
fi

porch_join=$(post_json /v1/spots/join '{"spot_id":"Porch"}' "$jared_token")
porch_hangout=$(jq -er '.hangout_id' <<<"$porch_join")
jared_snapshot=$(curl --silent --fail -H "authorization: Bearer $jared_token" \
  "$server_url/v1/snapshot")
jq -e --arg id "$porch_hangout" '
  (.spots[] | select(.name == "Porch") | .active_hangout_id == $id) and
  (.hangouts[] | select(.id == $id) | .label == "Porch")
' <<<"$jared_snapshot" >/dev/null
porch_conversation=$(jq -er \
  '.conversations[] | select(.label == "Porch" and .spot_id != null) | .id' \
  <<<"$jared_snapshot")
jq -e --arg id "$porch_conversation" '
  ([.conversations[] | select(.label == "Porch")] | length) == 1 and
  (.conversations[] | select(.id == $id) | .kind == "hangout")
' <<<"$jared_snapshot" >/dev/null
post_json /v1/messages "$(jq -cn --arg id "$porch_conversation" \
  '{conversation_id:$id,content_type:"text/plain",payload:"persistent porch message",encryption_version:0}')" \
  "$jared_token" >/dev/null

sqlite3 "$database" <<'SQL'
UPDATE messages
SET created_at = '2000-01-01T00:00:00Z'
WHERE payload = 'persistent porch message';
INSERT INTO hangouts(id, livekit_room, created_at, ended_at)
VALUES (
  '00000000-0000-4000-8000-000000000099',
  'wisp-retention-test',
  '2000-01-01T00:00:00Z',
  '2000-01-01T00:01:00Z'
);
INSERT INTO conversations(id, kind, label, hangout_id, created_at)
VALUES (
  'hangout:00000000-0000-4000-8000-000000000099',
  'hangout',
  'Old room',
  '00000000-0000-4000-8000-000000000099',
  '2000-01-01T00:00:00Z'
);
INSERT INTO conversation_members(conversation_id, user_id, joined_at)
VALUES (
  'hangout:00000000-0000-4000-8000-000000000099',
  '00000000-0000-4000-8000-000000000001',
  '2000-01-01T00:00:00Z'
);
INSERT INTO messages(
  id, conversation_id, sender_id, created_at, content_type, payload,
  encryption_version
)
VALUES (
  '00000000-0000-4000-8000-000000000098',
  'hangout:00000000-0000-4000-8000-000000000099',
  '00000000-0000-4000-8000-000000000001',
  '2000-01-01T00:00:00Z',
  'text/plain',
  'temporary room message',
  0
);
SQL
circle_conversation=$(jq -er '.conversations[] | select(.kind == "circle") | .id' \
  <<<"$jared_snapshot")
# Independent DM verifies image access and per-user close/clear across backup/restart.
history_conversation=$(post_json /v1/conversations/direct '{"friend":"Tyler"}' "$charlie_token" | jq -er '.id')
image_message=$(post_json /v1/messages/image "$(jq -cn --arg id "$history_conversation" \
  '{conversation_id:$id,png_base64:"iVBORw0KGgoAAAANSUhEUgAAAAIAAAACCAYAAABytg0kAAAAEUlEQVR4nGPQ7/n/H4QZYAwAXFAK5eZRdEUAAAAASUVORK5CYII=",caption:"image history test"}')" "$charlie_token")
image_id=$(jq -er '.id' <<<"$image_message")
curl --silent --fail -H "authorization: Bearer $tyler_token" "$server_url/v1/messages/$image_id/image" >/dev/null
if curl --silent --fail -H "authorization: Bearer $jared_token" "$server_url/v1/messages/$image_id/image" >/dev/null; then
  echo 'Non-member downloaded a chat image' >&2; exit 1
fi
file_message=$(post_json /v1/messages/file "$(jq -cn --arg id "$history_conversation" \
  '{conversation_id:$id,file_name:"notes.txt",data_base64:"cHJpdmF0ZSBub3Rlcw==",caption:"file history test"}')" "$charlie_token")
file_id=$(jq -er '.id' <<<"$file_message")
[[ $(curl --silent --fail -H "authorization: Bearer $tyler_token" "$server_url/v1/messages/$file_id/file") == 'private notes' ]]
if curl --silent --fail -H "authorization: Bearer $jared_token" "$server_url/v1/messages/$file_id/file" >/dev/null; then
  echo 'Non-member downloaded a chat file' >&2; exit 1
fi
curl --silent --fail -X PATCH -H "authorization: Bearer $charlie_token" -H 'content-type: application/json' \
  -d '{"text":"Edited attachment caption"}' "$server_url/v1/messages/$file_id" >/dev/null
post_json /v1/conversations/clear "$(jq -cn --arg id "$history_conversation" '{conversation_id:$id}')" "$tyler_token" >/dev/null
post_json /v1/conversations/tab "$(jq -cn --arg id "$history_conversation" '{conversation_id:$id,closed:true}')" "$tyler_token" >/dev/null
(
  for message_number in $(seq 1 40); do
    post_json /v1/messages "$(jq -cn --arg id "$circle_conversation" \
      --arg text "backup traffic $message_number" \
      '{conversation_id:$id,content_type:"text/plain",payload:$text,encryption_version:0}')" \
      "$jared_token" >/dev/null
    if (( message_number == 1 )); then touch "$test_dir/writer-started"; fi
  done
) &
writer_pid=$!
for _ in $(seq 1 100); do
  [[ -f "$test_dir/writer-started" ]] && break
  sleep 0.01
done
[[ -f "$test_dir/writer-started" ]]
WISP_DATABASE_URL="sqlite://$database" ./scripts/backup-database.sh "$test_dir/backup.sqlite3" >/dev/null
wait "$writer_pid"
writer_pid=""
[[ $(sqlite3 "$test_dir/backup.sqlite3" 'PRAGMA integrity_check;') == ok ]]
stop_server
sqlite3 "$database" "UPDATE users SET display_name='Damaged' WHERE display_name='Jared';"
WISP_DATABASE_URL="sqlite://$database" WISP_RESTORE_OFFLINE_CONFIRMED=1 \
  ./scripts/restore-database.sh "$test_dir/backup.sqlite3" >/dev/null
[[ $(sqlite3 "$database" "SELECT display_name FROM users WHERE id='00000000-0000-4000-8000-000000000001';") == Jared ]]
start_server

jared_token=$(new_session "$jared_device_id" "$jared_device_token" | jq -er '.token')
tyler_token=$(new_session "$tyler_device_id" "$tyler_device_token" | jq -er '.token')
curl --silent --fail -H "authorization: Bearer $tyler_token" "$server_url/v1/snapshot" \
  | jq -e --arg id "$history_conversation" '.conversations[] | select(.id == $id) | .tab_closed and .last_message == null and .unread_count == 0' >/dev/null
if curl --silent --fail -H "authorization: Bearer $tyler_token" "$server_url/v1/messages/$image_id/image" >/dev/null; then
  echo 'Cleared image became visible after restart' >&2; exit 1
fi
curl --silent --fail -H "authorization: Bearer $charlie_token" "$server_url/v1/messages/$image_id/image" >/dev/null
if curl --silent --fail -H "authorization: Bearer $tyler_token" "$server_url/v1/messages/$file_id/file" >/dev/null; then
  echo 'Cleared file became visible after restart' >&2; exit 1
fi
[[ $(curl --silent --fail -H "authorization: Bearer $charlie_token" "$server_url/v1/messages/$file_id/file") == 'private notes' ]]
curl --silent --fail -H "authorization: Bearer $charlie_token" "$server_url/v1/snapshot" \
  | jq -e --arg id "$file_id" '.messages[] | select(.id == $id) | .edited_at != null and .payload.caption == "Edited attachment caption"' >/dev/null
curl --silent --fail -X DELETE -H "authorization: Bearer $charlie_token" "$server_url/v1/messages/$file_id" >/dev/null
if curl --silent --fail -H "authorization: Bearer $charlie_token" "$server_url/v1/messages/$file_id/file" >/dev/null; then
  echo 'Deleted attachment remained downloadable' >&2; exit 1
fi
post_json /v1/messages "$(jq -cn --arg id "$history_conversation" '{conversation_id:$id,payload:"new DM after close"}')" "$charlie_token" >/dev/null
curl --silent --fail -H "authorization: Bearer $tyler_token" "$server_url/v1/snapshot" \
  | jq -e --arg id "$history_conversation" '.conversations[] | select(.id == $id) | (.tab_closed | not) and .last_message.payload == "new DM after close"' >/dev/null
curl --silent --fail -H "authorization: Bearer $tyler_token" \
  --get --data-urlencode "conversation_id=$conversation_id" "$server_url/v1/messages" \
  | jq -e 'length == 1 and .[0].payload == "offline hello"' >/dev/null
curl --silent --fail -H "authorization: Bearer $jared_token" \
  --get --data-urlencode "conversation_id=$porch_conversation" "$server_url/v1/messages" \
  | jq -e 'length == 1 and .[0].payload == "persistent porch message"' >/dev/null
curl --silent --fail -H "authorization: Bearer $jared_token" \
  --get --data-urlencode "conversation_id=hangout:00000000-0000-4000-8000-000000000099" \
  "$server_url/v1/messages" | jq -e 'length == 0' >/dev/null
curl --silent --fail -H "authorization: Bearer $jared_token" "$server_url/v1/snapshot" \
  | jq -e --arg conversation "$porch_conversation" '
      (.spots[] | select(.name == "Porch") | .active_hangout_id == null) and
      ([.hangouts[].label] | index("Porch") == null) and
      ([.conversations[] | select(.label == "Porch")] | length) == 1 and
      (.conversations[] | select(.id == $conversation) | .spot_id != null)
    ' >/dev/null

porch_rejoin=$(post_json /v1/spots/join '{"spot_id":"Porch"}' "$jared_token")
jq -e --arg previous "$porch_hangout" '.hangout_id != $previous' <<<"$porch_rejoin" >/dev/null
curl --silent --fail -H "authorization: Bearer $jared_token" "$server_url/v1/snapshot" \
  | jq -e --arg conversation "$porch_conversation" '
      ([.conversations[] | select(.label == "Porch")] | length) == 1 and
      (.conversations[] | select(.id == $conversation) |
        .last_message.payload == "persistent porch message")
    ' >/dev/null

curl --silent --fail -X DELETE -H "authorization: Bearer $tyler_token" \
  "$server_url/v1/devices/$tyler_device_id" >/dev/null
if new_session "$tyler_device_id" "$tyler_device_token" >/dev/null 2>&1; then
  echo "revoked device received a new session" >&2
  exit 1
fi

if rg 'offline hello|persistent porch message|temporary room message' "$test_dir/server.log" >/dev/null; then
  echo "message content appeared in normal server logs" >&2
  exit 1
fi

echo "Private-alpha messaging, Porch, device auth, restart, retention, backup, and restore tests passed."
