# Readable invitations (schema 31)

Invitations are still encrypted, one-use, and explicitly accepted. The default
and maximum lifetime is now 720 minutes (12 hours); clients may request a shorter
lifetime. The returned `expires_at` is authoritative. Preview never redeems an
invitation or joins voice. Existing v1/v2 invitations remain supported.

## Creation

1. POST `/v2/server-invites` as an authenticated member. The existing response
   additionally contains `short_origin` and `short_label`. Use exactly the returned
   origin: on the owner-managed production service it is `https://wisp.you`.
2. Create and upload the existing encrypted v2 invitation unchanged. Its embedded
   server and AES-GCM AAD stay bound to the canonical API origin, not the branded
   website. PUT `/v2/server-invites/{id}/envelope` as before.
3. Generate **12 cryptographically random decimal digits**, rejecting numeric
   strings containing `69` or `666`. Concatenate the returned label and digits,
   with no separator before the digits. Labels come from the curated `WORDS`
   allowlist in `crates/wisp-crypto/src/short_invitation.rs`. Reserve distinct
   active labels, never a numbered sequence under the same active word. If every
   one-word label is occupied, the server expands to hyphenated word phrases.
4. Encrypt the full v2 link using the readable-link scheme below. PUT
   `/v2/server-invites/{id}/short` with `{lookup_id,envelope}`. No readable code,
   v2 fragment secret, or media key is sent in that upload. The shared link/QR is
   `{short_origin}/{label}{digits}`. Only show it after both uploads succeed.
   The server rejects previously issued lookup IDs with `invite_code_used` (409).

Twelve digits intentionally provide more guessing resistance than examples such
as `tea44`. The reservation avoids two active links with the same word and
slightly different digits. A one-way fingerprint is retained permanently to
prevent exact reuse, while full expired invitation records and ciphertext are
removed by the one-minute cleanup worker. Backups retain their usual recovery
history. Used/revoked aliases stop resolving immediately and are cleaned up too.

## Portable cryptographic contract

- Code: lower-case curated label, optionally hyphen-separated words, then exactly
  12 digits. The complete code is case-sensitive; no query string or fragment.
- Key: Argon2id version 19, memory 19456 KiB, iterations 2, parallelism 1,
  output 32 bytes. Password is UTF-8 code; salt is UTF-8
  `wisp-short-invite-v1`.
- Lookup ID: base64url without padding of SHA-256 of UTF-8
  `wisp-short-invite-v1\n` followed by the raw 32-byte Argon2 key. Do not hash
  the code directly: that would allow fast offline guessing of stored hashes.
- Cipher: AES-256-GCM, fresh random 12-byte nonce and 16-byte tag.
- AAD: UTF-8 `wisp-short-invite-v1\n{short_origin}\n{lookup_id}` (no final newline).
- Plaintext: full existing v2 HTTPS link, including its fragment secret.
- Envelope: base64url without padding of nonce || ciphertext || tag.
- Independent Python Argon2/AES fixture:
  `tests/fixtures/invitations/readable-v1.json`. Its decrypted v2 URL matches the
  existing `v2.json` fixture. Never use fixture codes outside tests.

## Open and accept

Accept `https://wisp.you/code`, `wisp.you/code`, or just `code` in app invite input.
Expand shortened/bare forms to the branded HTTPS origin. Resolve
`GET {short_origin}/v2/short-invitations/{lookup_id}` (no authentication), receiving
`{envelope}`. Reject redirects and insecure public URLs. Decrypt locally, parse
and resolve the inner v2 invitation using the existing flow. Honor expiry and
server-side rejection of used/revoked links. Show server name plus Accept/Decline;
no membership write until explicit acceptance. Reuse a saved account for the
canonical API or offer sign-in/account creation before acceptance. Do not send
credentials to the branded website or another API origin.

The website landing page offers an explicit Open Wisp button using the existing
`wisp-invite:v2.BASE64URL(full-readable-url)` OS handler. Android should handle
that scheme and native Wisp chat-link taps now. Verified HTTPS App Links require
publishing the actual release signing-certificate association; do not publish a
debug certificate. Website JS performs no preview fetch or automatic redemption.

## Hosting

Configure the API's `WISP_INVITE_URL=https://wisp.you`. Import
`infra/caddy/wisp-invitations.caddy` and use `import wisp_invitations LOOPBACK_API`
in the branded site's block. The routes serve only the isolated invitation page,
its assets, and the encrypted alias resolver. Access logging for readable paths
is disabled. No analytics/third-party scripts, referrer forwarding, or caching.
The website TLS endpoint necessarily sees URL paths; treat readable codes as
private invitation secrets. The database stores only hashes and ciphertext.

## Friends directory

`GET /v1/people` returns `{id,display_name,relationship,server_member}` for self,
community peers, friends, and incoming/outgoing requests. Server member views
must require `server_member === true`. Keep all contacts available in the account
Friends view even when not in the selected server. Preserve server/account scope
on request actions and direct messages.
