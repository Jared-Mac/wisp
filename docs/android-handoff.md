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

## Queued: remove friends and additional Wisp emojis (2026-09-23)

Status: **Deferred; desktop/server implementation only. Do not port yet.**

- Desktop adds a confirmed **Remove friend** action to friend context menus and
  participant menus. Cancel does nothing; failures allow retry. Confirmations are
  bound to the initiating account/server and dismissed if that connection changes.
- `DELETE /v1/friends/{user_id}` removes only the authenticated account's mutual
  friendship and pending requests between that pair. Returns the same `{people}`
  directory as existing friend actions and emits `friendship_changed`. Idempotent
  after removal, including when a nonmember disappears from the directory. Self
  removal is 400; unauthenticated is 401. Older servers return 404. No migration.
- Preserve DMs/history, memberships, keys and account blocks. Removal does not
  block the account; a new request/acceptance can restore friendship. Use stable
  user IDs. Desktop daemon command is `remove_friend` with `server_id,user_id`.
- Desktop now includes the twelve Wisp Everyday assets already released on
  Android, with matching shortcodes/artwork. Six new bundled SVGs use the same
  `:wisp_NAME:` format: hug, music, gaming, bonk, melting, comfy. Older clients
  display unknown shortcodes as text. No upload, API or schema change for artwork.
- Follow-up: omit a DM from navigation only when `history_cleared_at` is set,
  `last_message` is null, `unread_count` is zero, and none of its members is a
  current friend on that account service. Keep the durable conversation and any
  already-open tile. A new message or restored friendship lists it again; new
  empty chats and chats with retained history stay listed. No new API/schema.
  See `ChatLogic.listConversation` and `WispBridge.listedConversations`.
- Sources: server `friendships.rs`, daemon `friendships.rs`, `WispFriendships.qml`,
  `components/RemoveFriendDialog.qml`, `ChatMarkup.js`, `assets/emojis/*.svg`.
  `AnchoredPicker.qml` also keeps desktop emoji/invite popups inside their window.
- Validation: authenticated/actor-scoped API removal, mutual state, idempotence,
  request cleanup, retained membership/history; linked-account daemon routing;
  four-theme confirmation/cancel/retry/disconnect UI; picker bounds on resized
  app/tray windows; shortcode insertion, reaction routing and all SVG loading.

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

## Queued: keep healthy connections during failed room admission (2026-10-15)

Status: **Deferred; desktop implementation only. Do not port yet.**

- A failed encrypted-room admission must not discard a successfully fetched server
  snapshot or put the whole account into a reconnect loop. Leave the affected
  admission pending; retain all signature/key verification and retry on refresh.
- Apply the same separation to primary and additional server accounts. Do not
  automatically join voice or publish media as part of recovery.
- API/schema: unchanged. Sources: `apps/wispd/src/main.rs` (`refresh_room_admissions`)
  and `apps/wispd/src/session_tests.rs` (rejected admission regression).
- Validation: isolated server returns HTTP 403 for the encryption directory;
  primary and linked snapshot refreshes still succeed, recovery succeeds, and
  microphone/camera/screen publishing remain off.

## Queued: friend-to-server invitations and Windows update parity (2026-10-15)

Status: **Requested; implementation pending. Do not port or release yet.**

- Friends should receive server invitations entirely inside Wisp, showing the
  server name and explicit Accept/Decline actions. Existing invitation links
  remain supported. Mobile implementation belongs to a later requested batch.
- Windows auto-updating has been requested with launch/background/manual checks
  and configurable automatic behavior. This entry does not announce a release.
- API/schema and source references will be updated when implementation is complete.

## Queued: browser link feedback (2026-10-15)

External chat links now display brief “Opened in browser” feedback and suppress
repeated clicks on the same link for two seconds. Different links still open
immediately, and a failed browser launch can be retried. Invite links continue
through the in-app invitation flow. No API/schema changes. Sources:
`quickshell/app/components/LinkFeedback.qml`, `MessageFeed.qml`, `VideoEmbed.qml`.
Validated by `scripts/test-link-feedback.py` using real QML and an isolated browser
launcher fixture. Android platform feedback remains deferred until a requested
batch; this does not announce an Android release.

## Queued: in-app invitations to friends (2026-10-15)

Desktop server invitation menus can send a one-use, 12-hour invitation directly
to an accepted friend through their encrypted DM. Link/QR invitations remain.
Recipients see a server card with Accept… (opens the existing verified invitation
preview/confirmation) and Decline (remembered on this device). Expired invitations
are disabled. No room is joined and no media is published by receiving/accepting
a server invitation. Friendship and server membership are verified by the existing
authenticated endpoints; failure to send revokes the generated invitation.

Compatibility: no schema/server API migration or new encrypted content type. New
daemon IPC command `send_server_invite` accepts `server_id`, `friend` (stable UUID),
and optional `expires_in_minutes` (server caps at 720). The signed/encrypted
`text/plain` payload is exactly `Server invitation: <single-line server name>\n`
`Expires: <RFC3339>\n<existing Wisp invite URL>`. Older clients display a usable
link; updated clients render the card. Preview must verify the actual invitation
before acceptance; never trust the display name in the DM as authority.

Sources: `apps/wispd/src/account_membership.rs`, `quickshell/app/ChatMarkup.js`,
`WispAccountActions.qml`, `components/ServerSelector.qml`, `ServerInvitationCard.qml`,
`MessageFeed.qml`. Validation: isolated encrypted delivery/friendship rejection
regression in `session_tests.rs`, parser tests in
`scripts/test-server-invitation-markup.cjs`, Windows headless card/preview tests.
Android port remains deferred; do not read/action this queue until an explicit batch.

Update-safety follow-up: account and invitation actions now keep the client busy,
so an automatic update cannot interrupt acceptance or invitation delivery. Source:
`WispAccountActions.qml`, `WispUpdates.qml`; verified by the isolated update-settings
UI regression. The Windows updater also requires a responsive native daemon before
confirming installation; otherwise the old portable package is restored. No API
or schema changes, and Android implementation remains deferred.

Invitation menu follow-up: the desktop popup repositions when its link/QR content
grows or the window resizes, keeping all actions within the viewport. Source:
`components/ServerSelector.qml`; headless real-QML validation covers direct friend
selection, link/QR expansion and a narrow window. No protocol or schema changes.

## Queued: desktop send feedback and stream/chat reliability (2026-10-21)

Status: **Deferred; do not port until an Android batch is requested.**

- Desktop text messages/replies appear immediately with Sending status; confirmed
  IDs reconcile and retire the local row; deletion/history clearing must never
  resurrect an optimistic placeholder. Failed/uncertain sends retain the draft and offer
  Retry for review, without automatic duplicate retransmission. Attachments keep
  their existing upload queue/progress behavior.
- Local daemon `send_message` now returns `{message_id}` for primary/linked servers
  (matching existing reply acknowledgements). No HTTP API or schema change; older
  clients may ignore the added result.
- Ignore stale snapshot sequence numbers within a connection; reset on reconnect.
  Serialize server refreshes and preserve local media state when publishing them.
  Keep chat models stable during audio/video telemetry, and avoid repeated privacy
  refresh work when server data has not changed.
- Wait for a confirmed video subscription before opening its viewer; an earlier
  queued snapshot must not immediately close a newly requested stream. Clipboard
  responses belong only to the initiating composer/request.
- Sources: `apps/wispd/src/{main,privacy_transfers}.rs`,
  `quickshell/app/WispBridge.qml`, `components/{ChatComposer,MessageFeed}.qml`.
- Validation: isolated daemon concurrency/send tests; ChatReliability.qml covers
  clipboard ownership, snapshot ordering, telemetry identity, immediate sends,
  ack-before/after-snapshot, failure/retry and disconnect. Stream lifecycle, chat
  workspace, replies/pins and unread regressions also pass.

## Queued: plugin documents and Valheim modpacks (2026-10-25)

- Desktop adds a Wasm plugin host and a Valheim modpack plugin. Members explicitly
  share immutable pack revisions as retained, encrypted `.wisp-plugin.json`
  attachments in an existing DM/group/server channel. Import and local profile
  installation each require a local user action. Wisp servers never run mods.
- Compatibility: no server schema change and no new chat content type. Existing
  Android clients retain ordinary file-download behavior. Envelope format 1 has
  `plugin`, `plugin_version`, `title`, and `data`; the authenticated attachment
  message supplies author/audience. Pack data includes exact dependency strings,
  archive SHA-256/size, revision, and parent digest. Never trust self-claimed
  authors or automatically install/execute anything from a document.
- A later owner-requested Android batch may add read-only pack inspection and
  plugin document handling; desktop profile installation is Linux-specific and
  is not an Android game installation feature. No Android port is requested now.
- Sources: `crates/wisp-plugins/`, `apps/wispd/src/plugins.rs`,
  `apps/wispd/src/plugin_downloads.rs`, `quickshell/app/WispPlugins.qml`,
  `quickshell/app/components/PluginsPage.qml`,
  `examples/plugins/shared-modpacks/`, `docs/plugin-system.md`.
- Validation: sandbox/package/archive Rust tests, four-theme QML scope/action
  tests, and the isolated two-account encrypted-sharing fixture in
  `scripts/test-plugins.py`. Record results with the eventual release.

## Queued: offline voice membership cleanup (2026-10-25)

Status: **Deferred. No Android release or port requested.**

- The server now sends standard WebSocket Ping frames every 15 seconds and expires
  unresponsive control sockets after 45 seconds. Keep the WebSocket implementation's
  standard Pong handling enabled; no new application message or HTTP API is needed.
- After the last control connection closes, preserve voice membership for at least
  30 seconds. Retire it only after LiveKit confirms the participant is absent.
  Active media, other connected devices, recent joins, reconnects during the check,
  and media API failures preserve membership. Empty rooms are ended and remaining
  clients receive the existing `hangout_changed` event. No schema migration.
- Sources: `apps/wisp-server/src/voice_presence.rs`, `lib.rs`,
  `member_moderation.rs`, `main.rs`. Tests cover multiple devices, reconnection
  races, new memberships, failed media queries and silent/responsive sockets.
- Desktop-only: the private Quickshell build permanently disables unused
  Hyprland/i3 IPC modules and uses runtime cache revision `web2`, fixing Qt 6.12
  meta-type build failures. No Android changes are required for that build fix.
