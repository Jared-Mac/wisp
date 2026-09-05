# Privacy hardening

The fresh public host and this local owner client now require encrypted chat.
The old test database remains separate and was not migrated. This document
records the security boundary and remaining operational work; it is not an
independent cryptographic audit.

## Threat model and decisions

- Protect message content, captions, original filenames and attachment bytes
  from the hosting provider and coordination/media servers. TLS and encrypted
  disks alone cannot provide this while the server processes plaintext.
- Client-held account encryption/signing identity, with a user-held recovery
  code for new/reinstalled devices. No server escrow or password-reset recovery.
  Loss of all devices and the recovery code means loss of encrypted history.
- User-selected onboarding: trust on first use (TOFU). Automatically pin each
  friend's initial public identity locally and reuse it across rooms; manual
  fingerprint comparison is optional, not a prerequisite for ordinary use.
  Block unexpected identity changes instead of silently replacing pins. This
  trusts the initial exchange and does not protect a first contact intercepted
  by an actively malicious provider. Losing trust pins on reinstall reopens that
  bootstrap risk, even if the account recovery key itself is restored.
- Seamless room changes are the target: reuse pinned friend identities and
  automatically update recipients for future messages. Authenticate membership
  changes with authorized clients' signatures rather than trusting a server-only
  roster; otherwise the provider could inject itself as a new member. The branch
  implements signed membership chains, TOFU pins, owner/admin changes and signed
  admission. Legacy unsigned mutations are blocked for signed rooms. Voice invites
  carry a signed admission offer, applied only when accepted. Joining voice alone
  does not authorize encrypted chat membership; an owner/admin must invite the
  participant. Stale offers fail closed and must be sent again.
  Initial friend account IDs and names are also remembered locally; arbitrary
  new server-supplied accounts are rejected. Explicit one-use friend invitations
  expand the local trusted-contact set under the selected TOFU model.
- Joining a room must not automatically re-encrypt or disclose prior history.
  Default to messages sent after admission; history sharing needs a separate
  explicit policy/action. Removing a member excludes them from future messages,
  but cannot revoke plaintext or keys they already received.
- Assuming an honest initial exchange and uncompromised participant devices and
  recovery keys, a later server compromise cannot decrypt stored ciphertext by
  changing the key directory. This does not retroactively protect pre-E2EE
  plaintext logs/backups, nor prevent participants from sharing their own copies.
- Use standard age multi-recipient encryption and Ed25519 sender signatures,
  not a new cipher. Bind server/account, conversation, message ID and purpose in
  signed content. This archival design is NOT a forward-secret ratchet: stolen
  recovery keys can unlock captured history. Do not claim Signal-level security.
- Stream large files into private temporary destinations; expose decrypted
  output only after authenticated EOF and matching signed file digest. Never
  silently fall back to plaintext on missing keys or decrypt/encrypt failures.
- Media's shared alpha key is separate. Generate a fresh client-held media key
  before migration; never put it or chat recovery material on OVH.
- The provider still observes IPs, routing/membership, timestamps, traffic sizes
  and uptime. Room labels/voice invites currently contain readable coordination
  metadata; review these explicitly rather than calling all metadata encrypted.

## Rollout and operational gates

1. Client encryption primitives and recovery tests.
2. TOFU key exchange/pinning, local key storage/export/import and key-change UI;
   authenticated automatic room membership updates without per-room key setup.
3. Complete integration: text, edits, images, streamed files, thumbnails, invite
   cards, notification previews; ciphertext-only server enforcement, with no
   legacy upload bypasses. Preserve authorization and sender authentication.
4. Isolated multi-client tests: recovery, wrong keys, tampering, conversation
   relocation, removed members, plaintext rejection, interrupted/retried files,
   metadata leakage, private temporary files and cleanup.
5. Fresh-history cutover (user selected). Do NOT upload the old plaintext
   database, attachments, backups, host environment or shared media key to OVH.
   Preserve the old installation as-is; starting fresh does not authorize deletion.
6. Hardened host: key-only SSH, least-privilege services, firewall, TLS, automatic
   security updates, private administration, minimal logs, encrypted backups to
   a client-held recovery recipient. Disable development sessions and remove
   bootstrap credentials after enrollment. Test backup restoration.
7. Coordinate each additional client enrollment and preserve a working local
   rollback. A code change alone never authorizes a remote deployment.

Large unlimited-size attachments require a capacity/quota strategy even though
messages have no product-level character cap.

## Current checkpoint

- New `wisp-crypto` crate: age streaming encryption, Ed25519 context-bound sender
  signatures, private recovery files, TOFU identity pins and signed roster chains.
- Client integration: encrypted text/edits and streamed attachments, authenticated
  image/download materialization, fail-closed key errors and recovery import/export.
- Server integration: opaque encrypted endpoints, strict-mode legacy rejection,
  signed membership validation and ciphertext upload storage/expiry.
- Isolated tests cover two clients, recovery, signed admission with no old-history
  disclosure, tampering, wrong keys, private files, legacy rejection and chunk uploads.
- Authenticated accounts initialize chat encryption automatically on connection.
  Private keys are durably saved locally before publishing the public identity;
  setup retries reuse the same keys. Missing or different existing identities
  require recovery, never silent replacement. Chat remains blocked until ready.
- Privacy is a separate Settings tab with status, optional recovery export, and
  recovery import. Opening Settings never exports a key or asks for a backup path.
- The fresh server, owner account, local recovery identity, public TLS endpoint,
  and strict ciphertext-only history are active. Bootstrap enrollment is disabled.

Additional implemented checks cover encrypted image/file upload and HTTP download,
signed voice-invite acceptance (including replay and forged offers), edits after
admission without redistributing old history, plaintext downgrade rejection and
unknown-account substitution. File retention uses original plaintext size. The
HTTPS launcher requires chat encryption locally and blocks media without a
client-held key. The private-host service templates are installed on the fresh
host; key-only SSH, the firewall, TLS, strict encryption mode, and bootstrap
removal have been runtime-checked.

Remaining operational gates include enrolling additional clients, exercising
cross-device recovery, testing encrypted backup restoration, and completing a
multi-user media call on the public host. The legacy test host/database remains
unchanged.
This code has not received an independent cryptographic review; passing isolated
tests is not a production security audit.

The user selected a fresh start on OVH because existing history is testing data.
No history migration is needed. This does not authorize deleting the old data
or copying plaintext logs/backups to OVH.

## Sources for the implementation boundary

- [age format](https://github.com/C2SP/C2SP/blob/main/age.md)
- [Rust age API](https://docs.rs/age/0.11.5/age/)
- [Ed25519-dalek API](https://docs.rs/ed25519-dalek/2.2.0/ed25519_dalek/)
- [OVH Canada VPS selection](https://www.ovhcloud.com/en-ca/vps/)
