# Server membership moderation

Owners and administrators can kick or ban members from **Server settings → People
and roles**, including members who are offline. Each action asks for confirmation.
**Banned members** lists existing bans and lets an administrator lift one.

- Kick removes membership; the account may rejoin with a new server invitation.
- Ban removes membership and blocks admission until lifted. Bans follow the
  account's stable ID across display-name or username changes.
- Unban permits a future invitation; it does not restore membership or join voice.
- The owner and the acting administrator cannot remove themselves. Only the owner
  can remove another administrator. The server checks these rules on every request.
- The account, friendships, DMs and message history are preserved. Server-admin
  access and selected-channel grants are removed; outgoing server invitations,
  pending room invitations/admissions and knocks are cancelled.

Membership revocation commits before voice disconnection. A temporary media
outage does not undo a kick or ban: pending disconnects survive restarts and retry
automatically. The UI reports when voice disconnection is still pending. Wisp
clients stop media and cancel reconnection when server membership is removed.

## API and compatibility

`GET /v1/server/settings` includes offline members, `member_moderation: true`, and
`bans: [{id, display_name, banned_at, reason}]`. Older servers without that capability
do not show the new controls.

`POST /v1/server/members/moderate` accepts `user_id`, `action` (`kick`, `ban`, or
`unban`), and optional `reason` (up to 280 printable characters). It returns
`{ok: true, media_pending: boolean}`. Moderation is scoped to the authenticated
account on that server. Failed authorization is 403, missing account is 404, and
an invalid reason is 400. Repeated requests are idempotent.

Migration 35 persists bans and pending media disconnects. Its membership trigger
also protects older admission paths. Banned invitation redemption fails before
consuming the invitation. Account sign-in continues to work independently of
server membership.

## Self-hosted media boundary

Wisp denies new media-token requests after removal and uses LiveKit's server-side
`RemoveParticipant`. The supplied reverse-proxy configuration additionally checks
every `/rtc` handshake through `GET /v1/livekit/admission`. It validates the media
JWT's signature, issuer, validity and room against current server/room membership.
This also rejects cached tokens after removal. Keep LiveKit's signaling port
inaccessible publicly; access must go through this proxy. Apply the updated
`infra/private-host/Caddyfile.example` signaling block when upgrading an existing
installation, after the new Wisp server is running.

Initial media grants expire after 60 seconds. Self-hosted LiveKit does not revoke
previously issued tokens, and its session refreshes can extend their lifetime;
the proxy admission check, rather than token expiry, closes that gap. See
[LiveKit token revocation](https://docs.livekit.io/frontends/reference/tokens-grants/).

## Validation

- `cargo test -p wisp-server member_moderation --lib`
- `cargo test -p wisp-server media_admission --lib`
- `WISP_TEST_CADDY=/path/to/caddy cargo test -p wisp-server media_proxy -- --ignored`
- `bash scripts/test-server-moderation-ui.sh`
- `bash scripts/test-media.sh` includes removing a connected client, verifying
  LiveKit removal, stopped publishing and no automatic reconnection.
- Existing server account, membership and chat tests.
