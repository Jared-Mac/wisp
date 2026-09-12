# Account-associated encrypted backup — design review draft

Status: draft for desktop/server/Android peer review. This is not deployed and
must not be described as an independently audited Wisp protocol.

## User outcome and boundaries

An existing trusted device enables account backup once, choosing a fresh password
that is used only by the new secure authentication flow. New devices then restore
through sign-in without the trusted device being online. Native clients perform
all password-based cryptography. No plaintext key escrow, silent identity
replacement, device-token sharing, or automatic room/media joins.

Backup contains the existing account encryption/signing identity, verified contact
keys, approved conversation recipients, pinned signed room heads, trusted names
and signed-profile revision floors, and the media key for this account's matching
server/network. It excludes device credentials, session tokens, attachments,
decrypted message caches, unrelated accounts and arbitrary filesystem paths.
Wisp currently decrypts historical chat through its account identity; backing up
keys does not restore messages already deleted by server retention.

## Cryptographic foundation

Use pinned `opaque-ke` 4.0.1 with its Ristretto255/TripleDH/SHA-512 suite and a
fixed Argon2id v0x13 profile (65,536 KiB, 3 passes, 1 lane). The 4.0.1 README
describes RFC 9807 compatibility. Its linked 2021 audit concerned 0.5.0 with fixes
in 1.2.0, not this version or Wisp's composition. Client and server use the same
versioned Rust wire implementation, including Android JNI.

Opaque authentication context binds the protocol version, canonical service
origin, network UUID and immutable sign-in username, canonicalized by trimming
and ASCII lowercasing. Existing SQL lookup and uniqueness use `COLLATE NOCASE`;
case variants must share the authentication identifier and secure-mode latch.
Public display names and
friend handles never participate in key derivation. Limit handshake sizes,
pending attempts and lifetimes; clear one-use state on consumption. Unknown
accounts receive indistinguishable dummy login responses. Clients never use a
login export key before authenticating the server. Registration relies on the
configured HTTPS origin and pins the OPAQUE server public key for later exchanges.
Reauthentication also binds credential generation, attempt, device and exact
operation digest. Saved native login state includes the original context; restoring
it under a different context fails before finish. The library's finish parameters
do not re-bind an already serialized server handshake.

Generate a random 256-bit vault key. Use AES-256-GCM with independent random
96-bit nonces and domain-separated HKDF-SHA-256 keys for payload encryption and
export-key wrapping. The OPAQUE client-only export key wraps the vault key; it
never goes to the server. Wrapping AAD binds the account, network, protocol and
credential generation. Payload AAD binds account, network, schema and revision.
The vault key stays stable across normal password changes. Never reuse a nonce.

A password-only encrypted backup remains dependent on password strength if the
complete authentication server is compromised. OPAQUE avoids exposing passwords
and export keys during normal authentication; it does not make weak passwords
immune to offline guessing. An optional separate high-entropy recovery credential
or trusted device remains a fallback, never server-readable escrow.

## Backup authenticity, concurrency and local storage

The account signing identity signs each manifest, including ciphertext digest,
revision, parent digest and scope. The server requires both a device session and
that signature for initial upload and updates. A stolen device token alone cannot
replace a vault or enroll a new OPAQUE credential over an existing identity.

Use conditional updates against the current revision/digest. Before updating,
clients decrypt and merge the latest authenticated state, retaining identities,
pins, approved membership, and the most recent verifiable roster/profile floors.
Conflicting identity pins or unverifiable room forks stop synchronization. They
never resolve by last-writer-wins. Existing clients reject a remote rollback
below their locally retained revision. A newly restored device cannot detect an
otherwise valid entire-history rollback without an external checkpoint; do not
claim transparency-log guarantees.

Use a typed bundle with a bounded encoded size (proposed 8 MiB), no ZIP/path
extraction. Validate the entire bundle and all signatures before importing. Fresh
restore uses private staging and atomic installation. Existing local state merges
only with the same identity; corruption, permission errors, unknown versions and
conflicts preserve both copies. Imported media remains stopped until explicit
user action. Background backup follows actual key/trust changes with bounded
retry/backoff and a visible last-success/error state; flush pending changes during
normal shutdown without making shutdown unbounded.

## Authentication transition

Legacy accounts and running device credentials keep working. Account backup is
opt-in migration from a trusted device, authorized by the old password and a
signature from the existing account identity. The new password is distinct and
never sent to a plaintext-password route. The registration record, encrypted
vault, wrapped key, and removal of the old password verifier commit atomically.
If migration fails, legacy access and current files remain intact.

Secure sign-in is an explicit native path with no automatic legacy fallback.
A separate clearly labelled classic-account path is allowed for unmigrated
accounts. Do not use an untrusted per-account capability response to decide that
it is safe to send a secure password to a legacy endpoint. Remember secure mode
locally and never downgrade it. Existing older clients continue their current
device sessions but cannot perform password-based operations for migrated
accounts; secure operations require updated clients.

New secure registration should atomically create the account, public identity,
OPAQUE credential and initial encrypted vault, returning a new device credential.
This avoids registering a password without durably saving the initial identity.
A missing backup for an account with an existing public identity is a recovery
state, never permission to generate another identity.

## Proposed API groups (names/JSON not yet frozen)

All paths are `/v3/`, JSON bodies, no query credentials, no response/request-body
logging, no-store. Binary OPAQUE messages and ciphertext use one fixed base64
encoding. Response errors are fixed codes, never arbitrary echoed bodies.

- `auth/info`: global protocol/suite/network capabilities, no account lookup.
- `auth/login/start`, `auth/login/finish`: username, credential request and one-use
  challenge/finalization. Finish returns the existing device-credential shape;
  the client retains its export key and then restores through authenticated APIs.
- `auth/register/start`, `auth/register/finish`: signup reservation, OPAQUE
  registration response/upload, initial public identity and signed encrypted
  vault; no server membership is required.
- `auth/migrate/start`, `auth/migrate/finish`: device-authenticated legacy-password
  verification, registration transcript, existing identity proof, initial vault.
- `auth/reauth/start`, `auth/reauth/finish`: OPAQUE proof bound to the existing
  device and operation purpose; returns a short-lived one-use reauthentication
  grant, not a new device credential.
- `auth/password/start`, `auth/password/finish`: consume a reauthentication grant
  and atomically replace the credential plus vault-key wrapper. Keep payload/key
  unchanged; require an unlocked client and identity proof.
- `accounts/vault/status`, `accounts/vault`: authenticated status/read and signed
  conditional writes. Ordinary sync cannot change the credential or wrapped key.
- `accounts/recovery-email`: authenticated enrollment using a reauthentication
  grant. Never send a secure password through the old recovery-email endpoint.
- `auth/reset/start`, `auth/reset/finish`: native OPAQUE registration authorized by
  a valid single-use email token. Update authentication while preserving the
  existing locked backup and its old wrapping generation. Never fabricate keys.

## Password reset is not decryption recovery

A normal password change from an unlocked client rewraps the same vault key
atomically with the new credential. Other devices with the cached vault key can
continue syncing without sharing device credentials or repeatedly entering a
password. Retain an encrypted recovery path for interrupted requests.

A forgotten-password reset may regain account authentication, but cannot decrypt
an existing backup without an unlocked device or the separate recovery material.
Preserve the old encrypted backup. The new credential's generation differs from
its wrapper, so clients enter an explicit locked state. An existing trusted device
can sign in with the new password and rewrap its retained vault key; a recovery
file can restore the identity but may lack newer trust state and media keys.
Never claim an email reset restores those missing secrets.

Existing v2 email request/verification can remain. Token inspection must indicate
when a native secure reset is required, and v2 plaintext password completion must
reject migrated accounts. Web pages must not collect a secure replacement
password. The native reset-link handoff and copy/paste fallback need an explicit
Android/desktop contract before release. Token-bearing launch arguments and
telemetry must not be logged. Old-password API operations must not expose a
silent downgrade path after migration.

## Review and validation gates

The implemented shared bundle retains desktop name/revision floors and legacy
recipient approvals alongside Android's full signed profiles. Legacy approvals
remain metadata; enabling backup does not introduce a new send-policy guard.
Unpinned first-use names may be retained at revision zero, but cannot grant trust.
Different identity pins, equal-revision conflicting names/profiles, unequal media
keys and different legacy approvals stop the merge. A newer room checkpoint needs
a verified connecting signed chain, even if both checkpoints have valid signatures.
A restored checkpoint alone does not authorize current room membership.

Vault manifests also form a signed parent chain. A client with a saved checkpoint
accepts equality or a verified chain of successors, never a higher revision alone.
Proofs can be fetched in bounded pages. A new device cannot independently detect a
server replaying an entire valid history when it has no external checkpoint.

Each detached bundle candidate must pass the complete portable size/structure
budget before local import: 8 MiB plaintext, 4096 entries per map/collection,
100,000 aggregate JSON values and 24 levels of nesting. Duplicate keys, including
escaped aliases and noncanonical UUID map keys, fail decoding. All supported
fields survive round trips through clients that do not consume them. Missing
fields contribute nothing and never delete retained state. No deletion protocol
or key rotation is inferred from absence.

Sensitive native types have no public Debug/serde implementation. Temporary raw
keys and plaintext buffers use zeroizing owners and a bounded fixed-capacity
writer avoids plaintext Vec reallocations. AES, GHASH and POLYVAL cleanup features
are explicitly enabled; this remains best effort. POLYVAL 0.6.2 autodetection uses
ManuallyDrop without a matching Drop, and its PMULL backend has a cleanup TODO.
Parser/compiler scratch and other copies also prevent a total-memory-erasure
guarantee. No unsafe dependency patch or unreviewed prerelease is introduced.

Before a committing request, native clients atomically save the secure-mode
latch, immutable scope/identity/key, prospective credential, exact request and
operation digest. Mark dispatch as outcome-unknown durably before network I/O.
A failed session probe is not proof of non-commit. Reconcile through a verified
receipt or a fresh secure session for the same account; verified supersession may
close an older journal without discarding keys. Never indefinitely strand a
never-committed signup: it may renew authorization for its exact staged signup
effect when account/name/device IDs remain free. Renewal preserves the original
registration request/response/upload and signature. Changed response/server pin
or conflicting reservation/committed identity fails closed. If registration had
not yet produced a complete effect, it is still pre-commit and may restart with
the same staged identity/key/scope and a privately re-entered password.

Prospective device IDs and 256-bit tokens are generated and staged by the native
client. Only SHA-256 of the UTF-8 token text is sent in start (lowercase hex in
signed bindings, converted from those same bytes to URL-safe unpadded base64 for
the existing device table). A successful exact finish activates it atomically;
pending IDs never authorize ordinary sessions, notifications or voice.

Post-reset rewrap uses an existing-device Unlock exchange with no mutation grant,
then constructs the wrapper and obtains exact-operation reauthentication. Keep the
password only in an expiring native handle bound to scope/device/pin/generation
across the two exchanges. A generation change cancels the operation safely.
Native reset handoff initially uses a token-free Open Wisp action followed by
explicit paste of the reset link/code. Browser pages never collect a secure
replacement password, and no token enters an OS launch argument.

Review the downgrade boundary, KDF/nonce/domain parameters, token/credential
binding, reset behaviour and cross-client merge/import semantics before freezing
wire JSON. Tests must cover actual desktop/Android protocol interoperability,
wrong passwords, dummy accounts, tampering, cross-account/server substitution,
replay/expiry, revoked sessions, conflicting pins, rollback, concurrent sync,
interrupted migration/password change, email reset without decryption, safe media
restore, and fresh remote sign-in with the first device offline. No real account
migration or password entry is performed by an agent. Existing users enable the
feature privately after the complete flow is released.

References: [OPAQUE RFC 9807](https://www.rfc-editor.org/rfc/rfc9807),
[opaque-ke documentation](https://docs.rs/opaque-ke/4.0.1/opaque_ke/).
