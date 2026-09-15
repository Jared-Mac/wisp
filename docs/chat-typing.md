# Typing indicators

Chat composers show who is actively typing, including DMs, group chats and server
channels. Activity expires after eight seconds without an update. Clearing a draft,
sending a message, leaving the editor or disconnecting clears it sooner. Loading a
saved draft does not announce activity. Multiple people appear in one short line.

Only actual edits renew activity, at most once every three seconds per composer.
No draft text is sent or stored on the server. Activity never creates unread counts,
notifications, messages, or full snapshot/media refreshes.

Authenticated `POST /v1/typing` accepts `{conversation_id, active}`. Current chat
membership and DM blocking rules apply. `chat_typing` events contain
`{conversation_id, user_id, display_name?, active, timeout_ms}`. The server supplies
identity, limits active pulses to one per two seconds, and authorizes each recipient
against current conversation access before delivery. A stop has `timeout_ms: 0`;
active leases last at most 8000 ms and receivers must expire them locally even if
no stop arrives. No database migration is needed. Clients opt in through `/v1/events?typing=true`;
older clients receive no typing traffic. New clients quietly tolerate an older
server returning 404.

Validation: `cargo test -p wisp-server typing::tests`,
`node scripts/test-typing.cjs`, `bash scripts/test-typing.sh`, and the main/tray chat
UI fixtures. `node scripts/test-typing-events.cjs target/debug/wisp-server`
checks real loopback event sockets, including opt-in compatibility and private
recipient filtering.
