# Accounts and invitations, release 30

Accounts and community membership are distinct on the account service. Existing
accounts retain membership and their UUIDs, credentials, histories, permissions,
and encryption identities. Public registration creates an account with no server
membership. Friends and encrypted direct/group chats can work without joining a
community. This is not cross-host federation: independent hosts retain their own
accounts. Do not forward an account credential to a different origin.

## Account discovery and authentication

`GET https://wisp.you/.well-known/wisp-account.json` returns `{"server":"https://…"}`.
The value is the existing canonical account API origin. Reject redirects, userinfo,
and insecure public origins. The desktop helper also accepts an explicit advanced
server address, or `WISP_ACCOUNT_SERVER` for isolated tests.

`GET /v2/accounts/capabilities` reports `accounts_without_membership`,
`server_invites: 2`, and `public_handles`.

`POST /v2/accounts/register` takes `username`, `display_name`, `password`,
`device_name`, `protocol_version: 1`. There is no invitation field. The lower-case
username is also the initial public friend handle. It returns the existing
`DeviceCredential`: `device_id`, `device_token`, `user` (`id`, `display_name`).
Login remains `POST /v1/accounts/login` with username/password/device_name and
protocol_version. Convert device credentials to a revocable session using
`POST /v1/sessions`, as existing clients do. These requests require HTTPS except
for local development. Existing Argon2id storage, session/device revocation,
password-work bounds and account validation remain in effect; public signup also
has a bounded rate limit.

Snapshots and per-server state include `server_member`. Missing fields from older
servers mean true for compatibility. A false value means retain the account
context for profile/friends/DMs but exclude it from the server picker. Do not
require a media key merely to sign in. Voice still requires membership and its
existing media key. Never join voice as a side effect of signup or acceptance.

## Public handles and friends

Authenticated routes:

- `GET /v2/accounts/me`: `{handle, server_member, blocked:[{id,display_name}]}`.
- `PUT /v2/accounts/handle`: `{handle}`; empty removes it from search.
- `POST /v2/people/lookup`: `{handle}` (optional leading `@`), exact match only;
  returns `{person:{id,display_name,handle}}` or `{person:null}`.
- Existing `/v1/friend-requests/{id}` and `/accept` retain explicit consent.
- `PUT /v2/people/{id}/block` removes friendship and requests; DELETE unblocks.
  Blocking rejects new DM writes and further requests. History is retained.

Legacy sign-in usernames are not published by migration. Their owners explicitly
choose a public username in Profile settings. `/v1/people` includes shared server
members, friends, and pending requests; accounts without membership cannot browse
the community directory. Nonmember event streams carry a generic `account_changed`
invalidation with no community event payload. Refresh relevant account state.

## Server invitations

These grant community membership only. They do not automatically make friends.
All invitation write routes require an authenticated member. Creation is rate
limited; invites are one-use, expiring, and revocable by creator or server admin.

1. `POST /v2/server-invites` with `{expires_in_minutes:30}` (1–1440) returns
   `{id,code,expires_at,server_name,inviter:{id,display_name},kind:"server"}`.
2. On the client, create the v2 capsule and upload it with
   `PUT /v2/server-invites/{id}/envelope`: `{lookup_id,envelope}`. Upload once.
3. Share `https://<canonical-origin>/join/#v2.<secret>` or its locally rendered QR.
4. Recipient derives the lookup ID and requests
   `GET /v2/invitations/{lookup_id}` → `{envelope}`. This does not redeem it.
5. Decrypt locally and validate the issuer, format and expiry. Show the server
   name, inviter, expiry and the account to be used. Accept only on explicit Join.
6. `POST /v2/server/join` with `{code}` and a session for this exact origin grants
   membership. Save the media key locally after success. Reuse an existing saved
   account at this origin; otherwise let the user sign in or create an account.
   Keep successful signup credentials if the invite expires during signup.

`GET /v2/server-invites` lists active invites (own, or all for an admin).
`DELETE /v2/server-invites/{id}` revokes. `POST /v2/server/leave` removes community
access while retaining the account and private conversations; the owner cannot
leave. Leaving revokes that user's outstanding invites. All room/media endpoints
and shared community channel access recheck membership.

## Capsule encoding (cross-platform contract)

All base64 is URL-safe, without padding. Secret: fresh 32 random bytes; nonce:
fresh 12 random bytes. HKDF-SHA256 salt is UTF-8 `wisp-server-invite-v2`, input key
material is the secret. Derive 32 bytes with info `lookup` for the public lookup
ID, and separately 32 bytes with info `encryption` for the AES-256-GCM key.

AAD is UTF-8, with literal LF separators:

```
wisp-server-invite-v2
<canonical HTTPS origin, no trailing slash>
<base64url lookup ID>
```

Plaintext is UTF-8 JSON `{v:2,server,id,code,server_name,inviter,expires_at,media_key}`.
The `media_key` may be null for local development. Production invitations include
the inviter's already-shared media key. The envelope is base64url of
`nonce || ciphertext || 16-byte GCM authentication tag`. It is at most 12,000
characters. No share secret, media key, or plaintext capsule goes to the relay.
Reject altered ciphertext, issuer mismatch, wrong versions, missing fields and
expired invitations. Do not fall back to unencrypted relay storage.

`tests/fixtures/invitations/v2.json` is a synthetic test vector produced with
Node's HKDF/AES-GCM implementation and checked independently by Rust. The source
implementation is `crates/wisp-crypto/src/invitation.rs`.

The HTTPS landing page has no analytics or third-party assets and sends no fragment
to the server. Its explicit Open Wisp action wraps the full link as
`wisp-invite:v2.<base64url(UTF-8 full HTTPS link)>` for the desktop URI handler.
The native helper also accepts the HTTPS link directly. Android should validate
and consume the same HTTPS format; verified App Links additionally require the
actual package/signing certificate association on the canonical origin. Until
that is configured, copy/paste remains available. No deferred deep-link behavior
is promised for manually installed APKs.

Legacy `wisp-invite:<base64url JSON v1>` links are still accepted through the older
flow, with a visible notice that legacy invitations also add their inviter as a
friend. New desktop server invitations always use v2.
