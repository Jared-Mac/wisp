# Android change queue — development only

**Collect changes here. Do not start an Android port until the owner explicitly
requests a batch update.** This file's existence, a pull, a desktop/server release,
or an FYI from another task is not permission to read/action the queue in the
Android task. Do not dispatch work or request acknowledgements automatically.

Every desktop/server change that affects Android must add or amend an entry here
before its commit. If there is no Android impact, no entry is necessary. Keep
entries concise and combine follow-up fixes into the same entry. Include behavior,
API/schema changes, compatibility, useful source paths and validation. Do not put
credentials, real accounts, private addresses, invite codes or media keys here.

When a batch is explicitly requested: read all queued entries, agree on the scope,
implement and test in the separate Android repository, and record the Android
release/commit next to completed entries. Desktop completion does not mark an
Android item complete. Announce only released Android work in patch notes.

This queue is internal coordination, not product documentation or a runtime
dependency. Remove it and its agent instructions from a public source release;
see [development workflow retirement](development-workflows.md).

## Queued: server kick, ban and unban (2026-09-15)

Status: **deferred Android — desktop/server implemented and tested; Android must wait**.
Desktop/server implementation: `b60b083a0d5d` (included in the main release).

- Server owners/admins can kick or ban offline as well as online members through
  Server settings → People and roles. Ban management has its own section.
- Confirmation is required. Ban reason is optional (280 printable characters).
  Owner/self cannot be removed; only the owner may remove another administrator.
- Kick removes membership; a new invite permits rejoining. Ban prevents admission
  until unbanned. Unban does not restore membership automatically. Account,
  friendships and DMs survive; server-admin access and selected-channel grants
  are removed. Use account IDs, never mutable display/login names, as targets.
- `GET /v1/server/settings` adds `member_moderation: true` and `bans` entries
  `{id, display_name, banned_at, reason}`; `members` already includes offline
  accounts. Hide moderation controls if the capability is absent on an older server.
- `POST /v1/server/members/moderate`: `{user_id, action: "kick"|"ban"|"unban",
  reason?: string}` → `{ok: true, media_pending: boolean}`. Normal authorization
  errors are 403; missing account is 404; invalid reason is 400. Pending media
  removal retries server-side; show that status without repeating the moderation.
- Membership/settings change events refresh directories and server settings.
  Clear removed-server room access and stop/cancel voice/media reconnection;
  keep the account signed in for friends/DMs. Never auto-join after unban.
- Pin confirmation and asynchronous results to the selected server/account.
- Database migration: `0035_server_moderation.sql`. See
  `apps/wisp-server/src/member_moderation.rs` and desktop
  `quickshell/app/views/ServerSettingsView.qml` for the completed contract.
- Media ingress checks current membership on `/rtc` handshakes, including cached
  token reconnects. Use the advertised media URL and obtain a new token through
  Wisp when joining; respect membership loss/403 without reconnect loops. Existing
  servers must deploy the updated Caddy signaling block alongside the server.
- Desktop validation: server and daemon unit tests, compact settings UI tests,
  Caddy cached-token rejection, and real LiveKit kick/disconnect integration passed.
- Validation to port: permissions, offline targets, ban persistence across rename
  and new sessions, invitation rejection without consuming it, unban/reinvite,
  confirmation/cancel, server switching, and media removal after an outage.

Android completion: **not started; await explicit batch instruction**.

## Queued: live chat typing (2026-09-15)

Status: **deferred Android — do not port until the owner requests a batch**.

- Show names of active typists per chat, keyed by stable user IDs, excluding self.
- POST `/v1/typing` with `{conversation_id, active}` on actual editor changes, no
  more than once per three seconds. Never transmit draft text or renew an idle draft.
- Subscribe through `/v1/events?typing=true`. Consume `chat_typing` events directly,
  without fetching a snapshot or generating
  unread counts/notifications: `{conversation_id,user_id,display_name?,active,
  timeout_ms}`. Expire locally within eight seconds; `active:false` clears at once.
- Stop on send, empty draft, leaving the editor, closing the view and disconnect.
  Clear received state on disconnect/access loss. Keep server/account scopes separate.
- Server checks current chat access and DM blocks for senders/recipients. DMs work
  for friends without server membership. No migration; tolerate 404 on old servers.
- Source: `apps/wisp-server/src/typing.rs`, daemon `chat_typing` routing,
  `quickshell/app/WispTyping.qml`, `TypingLogic.js`, `components/ChatComposer.qml`.
- Validation: server private-recipient/throttle/send/disconnect tests and desktop
  timeout, multiple-sender, self-filtering, server-isolation and composer fixtures.
