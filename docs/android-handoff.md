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

## Released on Android: server kick, ban and unban (2026-09-15)

Status: **Released in Android 0.7.0-test (build 9), 2026-09-22**.
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

Android implementation: `TLT26-churn/wisp-android` commit
`7e35071a480cd8ba57988819eec6d5189422df6e`, included in released 0.7.0-test (code 9).
Kotlin/server integration covers offline moderation, protected roles, rename/new
sessions, invite preservation, unban/reinvite, and retained friends/DMs. Confirmation
and account-switch UI tests compile; phone/emulator media-removal checks remain.
The signed test APK is published at https://wisp.you/android-test. See Android
`docs/android-0.7-verification.md` for the remaining device-test limits.

## Released on Android: live chat typing (2026-09-15)

Status: **Released in Android 0.7.0-test (build 9), 2026-09-22**.

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

Android implementation: `TLT26-churn/wisp-android` commit
`7e35071a480cd8ba57988819eec6d5189422df6e`, included in released 0.7.0-test (code 9).
Unit and loopback server integration tests pass for scoped expiry, editor throttling,
private recipients, opt-in, clear, blocked DMs, and no unread effects. Existing
WebSocket transport opts into typing; idle chats do not poll. Device UI testing remains pending; the signed test APK is published.

## Released: selectable Android chat layouts (2026-09-22)

Status: **Released in Android 0.7.0-test (build 9), 2026-09-22**.
`TLT26-churn/wisp-android` commit `ad6f1281536077b810691c3cba5a0117ad79f3c0`
adds all three layouts included in the 0.7.0-test (code 9) release. Per the owner's choice,
**Soft groups is Android's default**, including existing preferences without a
layout selection. Chat headers/composer and message/reaction spacing are smaller;
incoming messages stay left and sent messages right. Wisp tabs stay hidden in chat;
native navigation retains its black inset protection.

Android retains individual keyed messages for scrolling/attachment state, with
tap/long-press actions per message and the existing follow/unread badge behavior.
The full debug and preview JVM suites each passed 154 tests; the final UI adjustment
passed 10 focused checks per variant, APK builds and lint (0 errors). Instrumented
layout/action/scroll tests compile but have not run: the owner requires the emulator
to remain closed. Phone visual testing remains pending. The signed test APK is published. Backend
health/capabilities return HTTP 200; this client-only change requires no server delta.

- Appearance offers Grouped (desktop default), Compact, and Soft groups (Android default); persist selection
  independently of color/theme and avatar preferences. Desktop shares it across
  the main app and tray. No protocol or schema change.
- Grouped uses one avatar/header per sender run. Compact puts names inline with
  text and times in a left gutter, without chat avatars. Soft groups adds one
  subtle rounded background per sender run. Keep attachment/reply/forward context.
- Runs break on sender/server change, a five-minute gap, day boundary, invitations,
  or the unread divider. Deletions promote the next message to run start.
- Desktop hover/keyboard actions replace repeated permanent controls; Android
  uses per-message tap/long-press menus and keeps reaction chips visible.
- Source: `components/MessageFeed.qml`, `components/ReactionBar.qml`,
  `views/AppearanceSettingsView.qml`, `WispAppearance.qml`; behavior details in
  `docs/chat-message-layout.md` (QML paths relative to `quickshell/app`).
- Validation: all three layouts across four themes and narrow tray widths;
  selection persistence, sender/unread grouping, hover/focus actions, invitation
  transitions, attachments and existing message actions.

## Android 0.7.0-test publication evidence (2026-09-22)

- Final pushed source: `f8976e6c385db9a546476b15efb2729ddf33b5bd` in the private Android repository.
- Android checks: run `35767708295` succeeded (APKs, unit tests, lint and protocol checks).
- Published APK: `Wisp-0.7.0-test.apk`, build 9, 50,072,482 bytes.
  SHA-256: `c794ab47c63aa234f30d696e24e23e8f096d7418a8ad229b51781bc145685318`.
- Signature, app ID, SDK/ABI metadata, non-debuggable build, 16 KB alignment,
  public APK/hash, update manifest and app-link certificate association verified.
  Previous public distribution retained and backed up. No backend delta or restart.
- Also released: notification previews with an opt-out and in-app update downloads
  with integrity/signing verification and Android install confirmation.
- Device UI tests were compiled, not run; the emulator remained closed as requested.
- Discord coverage is tracked in `docs/release-tracking/android.json`; pending
  entries with confirmed workflow delivery already count as covered.
