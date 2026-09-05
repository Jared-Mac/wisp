#!/usr/bin/env bash
set -euo pipefail

repo_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
cd "$repo_dir"

test_dir=$(mktemp -d)
export XDG_CONFIG_HOME="$test_dir/config"
export WISP_ACCOUNTS_FILE="$test_dir/accounts.json"
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

if post_json /v1/dev/session '{"profile":"Owner"}' >/dev/null 2>&1; then
  echo "development login was available on a private-alpha server" >&2
  exit 1
fi

owner_credential=$(post_json /v1/devices/bootstrap "$(jq -cn \
  --arg token "$bootstrap_token" \
  '{bootstrap_token:$token,username:"owner",display_name:"Owner",password:"correct horse battery staple",device_name:"Host",protocol_version:1}')")
owner_device_id=$(jq -er '.device_id' <<<"$owner_credential")
owner_device_token=$(jq -er '.device_token' <<<"$owner_credential")
if post_json /v1/devices/bootstrap "$(jq -cn \
  --arg token "$bootstrap_token" \
  '{bootstrap_token:$token,username:"other-owner",display_name:"Other Owner",password:"another correct horse password",device_name:"Second host",protocol_version:1}')" \
  >/dev/null 2>&1; then
  echo "administrator bootstrap was accepted twice" >&2
  exit 1
fi
owner_session=$(new_session "$owner_device_id" "$owner_device_token")
owner_token=$(jq -er '.token' <<<"$owner_session")
owner_user_id=$(jq -er '.user.id' <<<"$owner_session")
jq -e '.protocol_version == 1 and .user.display_name == "Owner"' <<<"$owner_session" >/dev/null

invite=$(post_json /v1/account-invites \
  '{"kind":"friend","expires_in_minutes":5}' "$owner_token")
invite_code=$(jq -er '.code' <<<"$invite")
member_a_credential=$(post_json /v1/accounts/register "$(jq -cn \
  --arg invite "$invite_code" \
  '{invite_code:$invite,username:"member-a",display_name:"MemberA",password:"member a correct horse password",device_name:"MemberA laptop",protocol_version:1}')")
member_a_device_id=$(jq -er '.device_id' <<<"$member_a_credential")
member_a_device_token=$(jq -er '.device_token' <<<"$member_a_credential")
member_a_session=$(new_session "$member_a_device_id" "$member_a_device_token")
member_a_token=$(jq -er '.token' <<<"$member_a_session")
member_a_user_id=$(jq -er '.user.id' <<<"$member_a_session")

if [[ $(curl --silent --output /dev/null --write-out '%{http_code}' --http1.1 \
  -H 'connection: Upgrade' -H 'upgrade: websocket' \
  -H 'sec-websocket-version: 13' -H 'sec-websocket-key: d2lzcC10ZXN0LWtleQ==' \
  "$server_url/v1/events?token=$member_a_token") != 401 ]]; then
  echo "a device session was accepted in a query string" >&2
  exit 1
fi
if [[ $(curl --silent --output /dev/null --write-out '%{http_code}' \
  -H "authorization: $member_a_token" "$server_url/v1/snapshot") != 401 ]]; then
  echo "a non-Bearer authorization header was accepted" >&2
  exit 1
fi

if post_json /v1/accounts/register "$(jq -cn \
  --arg invite "$invite_code" \
  '{invite_code:$invite,username:"replay",display_name:"Replay",password:"replay correct horse password",device_name:"Replay",protocol_version:1}')" >/dev/null 2>&1; then
  echo "one-use invite was accepted twice" >&2
  exit 1
fi

conversation=$(post_json /v1/conversations/direct \
  '{"friend":"MemberA"}' "$owner_token")
conversation_id=$(jq -er '.id' <<<"$conversation")
post_json /v1/messages "$(jq -cn --arg id "$conversation_id" \
  '{conversation_id:$id,content_type:"text/plain",payload:"offline hello",encryption_version:0}')" \
  "$owner_token" >/dev/null

curl --silent --fail -H "authorization: Bearer $member_a_token" \
  --get --data-urlencode "conversation_id=$conversation_id" "$server_url/v1/messages" \
  | jq -e 'length == 1 and .[0].payload == "offline hello"' >/dev/null
curl --silent --fail -H "authorization: Bearer $member_a_token" "$server_url/v1/snapshot" \
  | jq -e --arg id "$conversation_id" \
      '.conversations[] | select(.id == $id) | .unread_count == 1' >/dev/null
post_json /v1/conversations/read "$(jq -cn --arg id "$conversation_id" \
  '{conversation_id:$id}')" "$member_a_token" >/dev/null

member_c_invite=$(post_json /v1/account-invites \
  '{"kind":"friend","expires_in_minutes":5}' "$owner_token")
member_c_credential=$(post_json /v1/accounts/register "$(jq -cn \
  --arg invite "$(jq -er '.code' <<<"$member_c_invite")" \
  '{invite_code:$invite,username:"member-c",display_name:"MemberC",password:"member c correct horse password",device_name:"MemberC laptop",protocol_version:1}')")
member_c_session=$(new_session \
  "$(jq -er '.device_id' <<<"$member_c_credential")" \
  "$(jq -er '.device_token' <<<"$member_c_credential")")
member_c_token=$(jq -er '.token' <<<"$member_c_session")
member_c_user=$(jq -er '.user.id' <<<"$member_c_session")
if curl --silent --fail -H "authorization: Bearer $member_c_token" \
  --get --data-urlencode "conversation_id=$conversation_id" "$server_url/v1/messages" \
  >/dev/null 2>&1; then
  echo "non-member read a private conversation" >&2
  exit 1
fi
# This fixture needs an independent MemberA/MemberC DM later in the test. Public
# servers do not create an all-users friend roster, so establish only that
# test-scoped relationship after verifying conversation membership isolation.
sqlite3 "$database" "INSERT INTO friendships(first_user_id,second_user_id,created_at) VALUES (MIN('$member_a_user_id','$member_c_user'),MAX('$member_a_user_id','$member_c_user'),datetime('now'));"

test_room=$(post_json /v1/rooms '{"name":"TestRoom"}' "$owner_token")
test_room_spot_id=$(jq -er '.spot_id' <<<"$test_room")
test_room_conversation=$(jq -er '.id' <<<"$test_room")
test_room_join=$(post_json /v1/spots/join "$(jq -cn --arg id "$test_room_spot_id" '{spot_id:$id}')" "$owner_token")
test_room_hangout=$(jq -er '.hangout_id' <<<"$test_room_join")
owner_snapshot=$(curl --silent --fail -H "authorization: Bearer $owner_token" \
  "$server_url/v1/snapshot")
jq -e --arg id "$test_room_hangout" '
  (.spots[] | select(.name == "TestRoom") | .active_hangout_id == $id) and
  (.hangouts[] | select(.id == $id) | .label == "TestRoom")
' <<<"$owner_snapshot" >/dev/null
jq -e --arg id "$test_room_conversation" '
  ([.conversations[] | select(.label == "TestRoom")] | length) == 1 and
  (.conversations[] | select(.id == $id) | .kind == "hangout")
' <<<"$owner_snapshot" >/dev/null
post_json /v1/messages "$(jq -cn --arg id "$test_room_conversation" \
  '{conversation_id:$id,content_type:"text/plain",payload:"persistent room message",encryption_version:0}')" \
  "$owner_token" >/dev/null

sqlite3 "$database" <<SQL
UPDATE messages
SET created_at = '2000-01-01T00:00:00Z'
WHERE payload = '"persistent room message"';
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
  '$owner_user_id',
  '2000-01-01T00:00:00Z'
);
INSERT INTO messages(
  id, conversation_id, sender_id, created_at, content_type, payload,
  encryption_version
)
VALUES (
  '00000000-0000-4000-8000-000000000098',
  'hangout:00000000-0000-4000-8000-000000000099',
  '$owner_user_id',
  '2000-01-01T00:00:00Z',
  'text/plain',
  '"temporary room message"',
  0
);
SQL
# Independent DM verifies image access and per-user close/clear across backup/restart.
history_conversation=$(post_json /v1/conversations/direct '{"friend":"MemberA"}' "$member_c_token" | jq -er '.id')
image_message=$(post_json /v1/messages/image "$(jq -cn --arg id "$history_conversation" \
  '{conversation_id:$id,png_base64:"iVBORw0KGgoAAAANSUhEUgAAAAIAAAACCAYAAABytg0kAAAAEUlEQVR4nGPQ7/n/H4QZYAwAXFAK5eZRdEUAAAAASUVORK5CYII=",caption:"image history test"}')" "$member_c_token")
image_id=$(jq -er '.id' <<<"$image_message")
curl --silent --fail -H "authorization: Bearer $member_a_token" "$server_url/v1/messages/$image_id/image" >/dev/null
if curl --silent --fail -H "authorization: Bearer $owner_token" "$server_url/v1/messages/$image_id/image" >/dev/null; then
  echo 'Non-member downloaded a chat image' >&2; exit 1
fi
file_message=$(post_json /v1/messages/file "$(jq -cn --arg id "$history_conversation" \
  '{conversation_id:$id,file_name:"notes.txt",data_base64:"cHJpdmF0ZSBub3Rlcw==",caption:"file history test"}')" "$member_c_token")
file_id=$(jq -er '.id' <<<"$file_message")
[[ $(curl --silent --fail -H "authorization: Bearer $member_a_token" "$server_url/v1/messages/$file_id/file") == 'private notes' ]]
if curl --silent --fail -H "authorization: Bearer $owner_token" "$server_url/v1/messages/$file_id/file" >/dev/null; then
  echo 'Non-member downloaded a chat file' >&2; exit 1
fi
curl --silent --fail -X PATCH -H "authorization: Bearer $member_c_token" -H 'content-type: application/json' \
  -d '{"text":"Edited attachment caption"}' "$server_url/v1/messages/$file_id" >/dev/null
post_json /v1/conversations/clear "$(jq -cn --arg id "$history_conversation" '{conversation_id:$id}')" "$member_a_token" >/dev/null
post_json /v1/conversations/tab "$(jq -cn --arg id "$history_conversation" '{conversation_id:$id,closed:true}')" "$member_a_token" >/dev/null
# Private rooms and chunked attachment data must survive a database backup/restore.
private_room=$(post_json /v1/rooms '{"name":"MemberA test room"}' "$member_a_token")
private_conversation=$(jq -er '.id' <<<"$private_room")
jq -e '.self_role == "host" and .can_clear_for_everyone and (.members | length == 1)' <<<"$private_room" >/dev/null
post_json /v1/rooms/invite "$(jq -cn --arg id "$private_conversation" --arg user "$member_c_user" '{conversation_id:$id,user_id:$user}')" "$member_a_token" >/dev/null
if post_json /v1/conversations/clear "$(jq -cn --arg id "$private_conversation" '{conversation_id:$id,for_everyone:true}')" "$member_c_token" >/dev/null 2>&1; then
  echo 'Ordinary member globally cleared room history' >&2; exit 1
fi
post_json /v1/rooms/admin "$(jq -cn --arg id "$private_conversation" --arg user "$member_c_user" '{conversation_id:$id,user_id:$user,admin:true}')" "$member_a_token" >/dev/null
curl --silent --fail -H "authorization: Bearer $owner_token" "$server_url/v1/snapshot" \
  | jq -e --arg id "$private_conversation" '[.conversations[] | select(.id == $id)] | length == 0' >/dev/null
upload_id=$(cat /proc/sys/kernel/random/uuid)
post_json /v1/file-uploads "$(jq -cn --arg id "$upload_id" --arg room "$private_conversation" '{id:$id,conversation_id:$room,file_name:"stream.txt",size:16,keep:true}')" "$member_a_token" \
  | jq -e '.received_bytes == 0 and .next_chunk == 0' >/dev/null
curl --silent --fail -X PUT -H "authorization: Bearer $member_a_token" --data-binary 'streamed payload' \
  "$server_url/v1/file-uploads/$upload_id/chunks/0" | jq -e '.received_bytes == 16' >/dev/null
chunked_message=$(post_json "/v1/file-uploads/$upload_id/complete" '{}' "$member_a_token" | jq -er '.id')
[[ $(post_json "/v1/file-uploads/$upload_id/complete" '{}' "$member_a_token" | jq -er '.id') == "$chunked_message" ]]
[[ $(curl --silent --fail -H "authorization: Bearer $member_c_token" "$server_url/v1/messages/$chunked_message/file") == 'streamed payload' ]]
backup_conversation=$(post_json /v1/conversations/group "$(jq -cn \
  --arg request_id "$(cat /proc/sys/kernel/random/uuid)" \
  --arg member_a "$member_a_user_id" --arg member_c "$member_c_user" \
  '{request_id:$request_id,name:"Backup traffic",members:[$member_a,$member_c]}')" \
  "$owner_token" | jq -er '.id')
(
  for message_number in $(seq 1 40); do
    post_json /v1/messages "$(jq -cn --arg id "$backup_conversation" \
      --arg text "backup traffic $message_number" \
      '{conversation_id:$id,content_type:"text/plain",payload:$text,encryption_version:0}')" \
      "$owner_token" >/dev/null
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
sqlite3 "$database" "UPDATE users SET display_name='Damaged' WHERE display_name='Owner';"
WISP_DATABASE_URL="sqlite://$database" WISP_RESTORE_OFFLINE_CONFIRMED=1 \
  ./scripts/restore-database.sh "$test_dir/backup.sqlite3" >/dev/null
[[ $(sqlite3 "$database" "SELECT display_name FROM users WHERE id='$owner_user_id';") == Owner ]]
start_server

owner_token=$(new_session "$owner_device_id" "$owner_device_token" | jq -er '.token')
member_a_token=$(new_session "$member_a_device_id" "$member_a_device_token" | jq -er '.token')
curl --silent --fail -H "authorization: Bearer $member_c_token" "$server_url/v1/snapshot" \
  | jq -e --arg id "$private_conversation" '.conversations[] | select(.id == $id) | .self_role == "admin" and .can_clear_for_everyone' >/dev/null
curl --silent --fail -H "authorization: Bearer $member_a_token" "$server_url/v1/snapshot" \
  | jq -e '.self.hangout_id == null' >/dev/null
[[ $(curl --silent --fail -H "authorization: Bearer $member_c_token" "$server_url/v1/messages/$chunked_message/file") == 'streamed payload' ]]
post_json /v1/conversations/clear "$(jq -cn --arg id "$private_conversation" '{conversation_id:$id,for_everyone:true}')" "$member_c_token" >/dev/null
[[ $(sqlite3 "$database" "SELECT COUNT(*) FROM messages WHERE id='$chunked_message';") == 0 ]]
[[ $(sqlite3 "$database" "SELECT COUNT(*) FROM file_uploads WHERE id='$upload_id';") == 0 ]]
[[ $(sqlite3 "$database" "SELECT COUNT(*) FROM file_chunks WHERE upload_id='$upload_id';") == 0 ]]
if curl --silent --fail -H "authorization: Bearer $member_a_token" "$server_url/v1/messages/$chunked_message/file" >/dev/null; then
  echo 'Room clear retained a kept file' >&2; exit 1
fi
curl --silent --fail -H "authorization: Bearer $member_a_token" "$server_url/v1/snapshot" \
  | jq -e --arg id "$history_conversation" '.conversations[] | select(.id == $id) | .tab_closed and .last_message == null and .unread_count == 0' >/dev/null
if curl --silent --fail -H "authorization: Bearer $member_a_token" "$server_url/v1/messages/$image_id/image" >/dev/null; then
  echo 'Cleared image became visible after restart' >&2; exit 1
fi
curl --silent --fail -H "authorization: Bearer $member_c_token" "$server_url/v1/messages/$image_id/image" >/dev/null
if curl --silent --fail -H "authorization: Bearer $member_a_token" "$server_url/v1/messages/$file_id/file" >/dev/null; then
  echo 'Cleared file became visible after restart' >&2; exit 1
fi
[[ $(curl --silent --fail -H "authorization: Bearer $member_c_token" "$server_url/v1/messages/$file_id/file") == 'private notes' ]]
curl --silent --fail -H "authorization: Bearer $member_c_token" "$server_url/v1/snapshot" \
  | jq -e --arg id "$file_id" '.messages[] | select(.id == $id) | .edited_at != null and .payload.caption == "Edited attachment caption"' >/dev/null
curl --silent --fail -X DELETE -H "authorization: Bearer $member_c_token" "$server_url/v1/messages/$file_id" >/dev/null
if curl --silent --fail -H "authorization: Bearer $member_c_token" "$server_url/v1/messages/$file_id/file" >/dev/null; then
  echo 'Deleted attachment remained downloadable' >&2; exit 1
fi
post_json /v1/messages "$(jq -cn --arg id "$history_conversation" '{conversation_id:$id,payload:"new DM after close"}')" "$member_c_token" >/dev/null
# MemberC clears too: only the prefix already cleared by MemberA is pruned.
post_json /v1/conversations/clear "$(jq -cn --arg id "$history_conversation" '{conversation_id:$id}')" "$member_c_token" >/dev/null
[[ $(sqlite3 "$database" "SELECT COUNT(*) FROM messages WHERE id='$image_id';") == 0 ]]
[[ $(sqlite3 "$database" "SELECT COUNT(*) FROM chat_images WHERE message_id='$image_id';") == 0 ]]
[[ $(sqlite3 "$database" "SELECT COUNT(*) FROM messages WHERE conversation_id='$history_conversation' AND json_extract(payload, '$')='new DM after close';") == 1 ]]
curl --silent --fail -H "authorization: Bearer $member_a_token" "$server_url/v1/snapshot" \
  | jq -e --arg id "$history_conversation" '.conversations[] | select(.id == $id) | (.tab_closed | not) and .last_message.payload == "new DM after close"' >/dev/null
curl --silent --fail -H "authorization: Bearer $member_a_token" \
  --get --data-urlencode "conversation_id=$conversation_id" "$server_url/v1/messages" \
  | jq -e 'length == 1 and .[0].payload == "offline hello"' >/dev/null
curl --silent --fail -H "authorization: Bearer $owner_token" \
  --get --data-urlencode "conversation_id=$test_room_conversation" "$server_url/v1/messages" \
  | jq -e 'length == 1 and .[0].payload == "persistent room message"' >/dev/null
curl --silent --fail -H "authorization: Bearer $owner_token" \
  --get --data-urlencode "conversation_id=hangout:00000000-0000-4000-8000-000000000099" \
  "$server_url/v1/messages" | jq -e 'length == 0' >/dev/null
curl --silent --fail -H "authorization: Bearer $owner_token" "$server_url/v1/snapshot" \
  | jq -e --arg conversation "$test_room_conversation" '
      (.spots[] | select(.name == "TestRoom") | .active_hangout_id == null) and
      ([.hangouts[].label] | index("TestRoom") == null) and
      ([.conversations[] | select(.label == "TestRoom")] | length) == 1 and
      (.conversations[] | select(.id == $conversation) | .spot_id != null)
    ' >/dev/null

test_room_rejoin=$(post_json /v1/spots/join "$(jq -cn --arg id "$test_room_spot_id" '{spot_id:$id}')" "$owner_token")
jq -e --arg previous "$test_room_hangout" '.hangout_id != $previous' <<<"$test_room_rejoin" >/dev/null
curl --silent --fail -H "authorization: Bearer $owner_token" "$server_url/v1/snapshot" \
  | jq -e --arg conversation "$test_room_conversation" '
      ([.conversations[] | select(.label == "TestRoom")] | length) == 1 and
      (.conversations[] | select(.id == $conversation) |
        .last_message.payload == "persistent room message")
    ' >/dev/null

curl --silent --fail -X DELETE -H "authorization: Bearer $member_a_token" \
  "$server_url/v1/devices/$member_a_device_id" >/dev/null
if new_session "$member_a_device_id" "$member_a_device_token" >/dev/null 2>&1; then
  echo "revoked device received a new session" >&2
  exit 1
fi

if rg 'offline hello|persistent room message|temporary room message' "$test_dir/server.log" >/dev/null; then
  echo "message content appeared in normal server logs" >&2
  exit 1
fi

echo "Private-alpha messaging, TestRoom, device auth, restart, retention, backup, and restore tests passed."
