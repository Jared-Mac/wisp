# Native account backup API v1 — integration contract

Status: frozen for implementation after desktop/Android peer review on 2026-09-12,
native foundation commit `bb32102`; not deployed. The shared Rust `account_vault` types are the
cryptographic source of truth. This document names the HTTP fields and transaction
rules before desktop/server/Android wiring. No client may guess a missing route
and fall back to sending its secure password through v1/v2.

## Common rules

All routes below use `/v3/`. JSON field names are snake_case; unknown/duplicate
fields and unsupported versions fail. HTTP bodies are bounded before parsing.
Responses are `Cache-Control: no-store`, `Referrer-Policy: no-referrer`. Logs
contain neither request/response bodies nor authorization values. Error bodies
use fixed `code`/`message` pairs; clients display their own fixed messages.

UUIDs are canonical lowercase hyphenated strings; binary OPAQUE/envelope fields
use padded standard base64. SHA-256 fields use 64 lowercase hex characters.
Passwords are 12 characters minimum, 1024 UTF-8 bytes maximum. UI input necessarily
enters native code (prefer mutable bytes over immutable JNI strings). New passwords
and OPAQUE export keys are never returned in native output, sent over HTTP or
serialized durably; minimize UI copies with best-effort cleanup.
Sign-in username is trimmed ASCII lowercase, matching existing SQL NOCASE lookup.

`Scope`, `AccountContext`, `LoginContext`, `Intent`, `DeviceBinding`, `Precondition`,
`Operation`, `SignupStartBinding`, `RegistrationTranscript`, `ResetEffect`, `KeyEnvelope`, `Envelope`, `SignedManifest`
and `Checkpoint` mean their exact shared Rust serialization. Signatures/digests
are computed by the shared functions, not ad-hoc client JSON ordering.

Global info is `GET /v3/auth/info`:
`{version:1,suite,origin,network,max_vault_plaintext_bytes:8388608}`.
Origin must equal the configured canonical account-service origin. A saved
network/server public-key pin cannot change based on a capability response.
There is no unauthenticated per-user secure/classic capability lookup.
The explicit, separately labelled classic-account flow remains available for
unmigrated accounts. Its separate input must never receive a secure password or
be entered automatically on an error. A retained secure-mode latch prevents
downgrade, including through a case variant of the same username.

Public start endpoints have per-peer and global rate limits, bounded concurrency,
and bounded pending-state counts. Login/reauth/unlock attempts expire after three
minutes. Registration authorizations expire after ten minutes. Mutation grants
expire after two minutes. Failure/expiry never establishes non-commit to a client.

## Sign-in and existing-device proofs

`POST /auth/login/start` request:
`{attempt,username,device:{kind:"prospective",id,token_sha256},device_name,request}`.
The client generates/stages the device UUID and a 256-bit random bearer token
first. It sends SHA-256 of the token's UTF-8 text, not the token or random bytes.
The shared conversion helper converts hex digest bytes to existing device-table
URL-safe unpadded base64. No pending device ID authorizes ordinary API access.

Response: `{context:LoginContext,response,expires_at}`. Context has `sign_in`
intent bound to the exact prospective device/hash and credential generation.
Unknown/classic accounts use the library's dummy password file, equally shaped
responses and stable secret-derived dummy account/generation inputs. No real
account UUID/profile is disclosed at start. Existing device IDs cannot be reused
or replaced. The native client verifies all requested context fields and its
retained OPAQUE server pin before producing finalization/export key.

`POST /auth/login/finish`: `{attempt,finalization}`. The one-use successful proof
activates that exact device hash in the same transaction. Response:
`{device_id,user:{id,display_name},scope,username,identity,credential_generation}`.
The client assembles its device credential from its previously staged token;
the response never needs to resend it. Establish a normal `/v1/sessions` session
and restore/validate the authenticated vault before encryption is usable. If the
credential generation differs from the wrapper generation, sign-in succeeds into
an explicit **authenticated, backup locked** state: allow account/recovery status,
receipts, reset and trusted-device rewrap; block message encryption, enrollment,
replacement identity creation and automatic media/room joining. Do not describe
this as a wrong-password failure or silently enter classic enrollment.

`POST /auth/unlock/start`: authenticated `{attempt,request}`. The server derives
account and existing device from the session, returning the same started shape
with an `unlock` intent. `/auth/unlock/finish` accepts `{attempt,finalization}` and
returns `{confirmed:true,credential_generation}`. It never creates a device or
grant. This permits constructing a post-reset wrapper before a body-bound proof.

`POST /auth/reauth/start`: authenticated `{attempt,request,operation:Operation}`.
The existing device/account and current generation must match the operation.
Response uses `reauthenticate` intent with its exact `Operation::digest()`.
`POST /auth/reauth/finish`: authenticated `{attempt,finalization}` returns
`{grant,operation_sha256,expires_at}`. The random single-use grant is hashed at
rest and bound to account, device, generation, purpose and entire operation.
Finish/consumption recheck the active session/device and all preconditions.

The native password handle is short-lived and bound to origin/network/account,
username, device, expected pin and generation. Post-reset rewrap uses Unlock,
constructs the wrapper, then reauthenticates its exact operation without another
password prompt. Cancellation/expiry/account change drops the handle; it is never
serialized. Expensive crypto/network work happens outside the repository lock.

### Ending an outcome-unknown login

After fresh secure sign-in restores the same account on a different staged
credential, `POST /auth/login/terminate` accepts authenticated
`{attempt,device:DeviceBinding::Prospective}` for the original unknown sign-in.
It atomically cancels that pending proof, revokes the exact account/device/token
hash if already activated, and saves permanent **account-scoped** terminal
state. Response `{terminated:true,attempt,device_id,scope,revoked}` must match all
saved bindings before the client retires the original stage; `revoked` alone is
not the verdict. It never returns or reissues tokens. Exact retries return the
same terminal result. The current recovery device cannot terminate itself.

Start and finish cannot reactivate a terminated attempt for that account,
including through an old completion receipt. Missing/expired pending state still
receives a tombstone, preventing a delayed matching start or finish. Tombstones
are keyed by account plus attempt; creating one cannot preempt another account's
start using that attempt UUID. Existing foreign account/kind/device/hash state
is rejected. Creation (including missing rows) is rate limited per account/peer.
Signup recovery first verifies the exact committed enrollment receipt, then
revokes or deliberately retains its original device; signup receipt retries
never recreate or reactivate that credential.

## Secure signup and enabling an existing account

New signup stages an identity, vault key, proposed account UUID, generation UUID,
prospective credential and operation UUID before registration. It needs no server
membership or invite. `POST /auth/register/start`:
`{operation_id,scope,generation,username,display_name,device,device_name,request,identity,signature,previous_binding_sha256}`.
All names/IDs must be free; the scope origin/network must match this server.
This reserves registration without creating a user or authorized device.
The signature is `SignupStartBinding::sign()` by the proposed identity. The server
constructs that shared binding from these fields, format1, id=operation_id,
metadata in signup and `auth::registration_request_digest(request)` (lowercase
hex SHA-256 of validated canonical padded-base64 request UTF-8 **text**).
Initial previous_binding_sha256 is null. Public IDs and token hashes alone cannot
create or replace a reservation for that identity. A pre-commit replacement
requires the same identity, scope, operation, generation, metadata and device,
with previous_binding_sha256 equal to the current whole binding digest. Exact
current-binding retries are idempotent; delayed older bindings and competing
replacements fail. Clients stage the signed binding before start and never
replace it after a finish may have been dispatched.

Started response:
`{operation_id,account:AccountContext,scope,generation,expected:Precondition,response,authorization,expires_at}`.
Authorization is an unpredictable HMAC-SHA256 ticket, hashed at rest, using a
private server key and a versioned domain-separated encoding of the complete
start binding digest and exact stored integer expiry. An active exact retry,
including after server restart, returns the same authorization/expiry/response;
it never rotates an in-flight authorization. Expired uncommitted renewals can
choose a new expiry after checking uniqueness, committed state and binding.
No plaintext ticket, password or private identity is stored in pending JSON. The server's OPAQUE response for the same setup/credential identifier/
registration request is deterministic. Client identity/key/scope remain fixed.

`POST /auth/register/finish` body:
`{authorization,operation,signature,registration:RegistrationTranscript,wrapper,vault}`.
Operation is `enroll`, prospective device, empty precondition, and includes
`signup:{username,display_name,device_name}`. Its registration digest covers
original request/response/upload/context/scope/generation; wrapper and first
signed vault checkpoint digests must match. Its signature is made by the vault's
account identity. One transaction creates user (`server_member=0`), public
identity, secure credential, encrypted vault/wrapper, first manifest and device,
plus an idempotent committed-effect receipt. No raw password verifier is created.
Response matches successful login finish.

Enabling a classic account is device-authenticated:
`POST /auth/migrate/start` takes
`{operation_id,generation,request,legacy_password}` and verifies the current
legacy password. The client requires a different fresh secure password privately.
Only the legacy password is sent here; it must never be stored in pending JSON,
logs, or a fast request-body digest. The saved authorization instead binds the
existing slow verifier fingerprint, account/device and nonsecret request fields.

`POST /auth/migrate/finish` has the same shape as signup finish, but `enroll`
has an existing device, `signup:null`, and uses the already published identity.
Atomically write the initial vault/wrapper/OPAQUE credential and remove the old
password verifier. Existing device sessions remain valid. A stolen session alone
cannot migrate because legacy password and existing-identity proof are required.

Before a finish can be dispatched, store the complete signed effect, exact retry
body and all private restore material atomically, then mark outcome unknown.
If a final response is lost, the previously staged prospective credential can
probe `/v1/sessions`; 401/404 is never proof of failure/non-commit. Exact retries
compare immutable effect digest, independent of ephemeral authorization bytes.

An expired uncommitted signup may renew start authorization for the same saved
effect if all account/name/device IDs remain free. It must preserve original
request/response/upload, scope, identity, key, metadata and signature; any changed
deterministic response/pin fails. A deleted reservation does not prove a prior
binding; renewal is a new authorization subject to all uniqueness checks. It
never overwrites a conflicting completed or active reservation. Pre-commit
restart (before a complete effect exists) may restart native registration with a
privately re-entered password while retaining proposed scope/identity/key/device.

## Vault reading, proofs and concurrent writes

`GET /accounts/vault/status` is authenticated and returns:
classic `{mode:"classic",scope,username,identity}` or secure
`{mode:"secure",scope,username,identity,credential_generation,wrapper_generation,vault:Checkpoint}`.
The identity may be absent only for a classic account not yet enrolled in E2EE.
A missing secure vault is an error/recovery state, never permission to generate
another identity or use legacy automatic enrollment.

`GET /accounts/vault` returns `{state,wrapper,vault:Envelope}`. Save the target
envelope/checkpoint while checking history. `POST /accounts/vault/proof` accepts
`{after:Checkpoint,through:Checkpoint}` and returns up to128 signed manifests,
`{manifests,next:Checkpoint,complete}`. The server requires exact stored hashes;
each native client verifies scope/identity/signatures and every parent link.
Continue from next to the fixed target. A fresh restore checks the latest signed
envelope and authenticated decryption, with the documented lack of an external
rollback checkpoint. Existing clients never skip their retained checkpoint.

A proof page must make nonempty forward progress unless already at the target;
`next` equals the last locally verified checkpoint and `complete` is true exactly
when next equals the fixed target. Cap a page at128 manifests and512KiB, and one
foreground catch-up batch at32 pages/30 seconds. An unfinished batch preserves
its target and verified progress for bounded background continuation; it never
accepts a gap or discards a retained checkpoint to catch up faster.

`POST /accounts/vault` accepts `{operation,signature,vault}`. Operation is
`store_vault`, signed by the existing identity, with exact current credential/
wrapper generations and vault checkpoint. The new signed manifest must be its
immediate successor. Compare preconditions and commit vault/manifest/receipt in
one transaction. Ordinary sync cannot change the wrapper or credential.

On conflict, fetch/decrypt/merge latest state outside the live-state lock. Preserve
all fields and obtain connecting room proofs for different room checkpoints.
Validate complete merged portable bounds. Durably save a detached candidate plus
dirty generation before reporting trust changes as saved; publish in-memory state
only after atomic persistence and stale-generation checks. Background retries
are bounded and resume on foreground/reconnect; show last-success/error status.

## Password change and repairing a locked wrapper

`POST /auth/password/start` is authenticated:
`{operation_id,generation,request,expected:Precondition}`. It returns registration
started shape, binding the current credential/wrapper/vault state. Prepare the
new native registration/wrapper using the retained vault key, then reauthenticate
the exact `change_password` operation with the old password.

`POST /auth/password/finish`:
`{authorization,grant,operation,signature,registration,wrapper}`.
Atomically replace credential plus wrapper, keep payload/vault key/identity,
consume grant/authorization, and save the exact-effect receipt. Concurrent sync,
password changes and resets must not bypass the precondition check.

Expired migration/password-start authorizations may be renewed for the same saved
registration request and immutable effect only while their expected account state
still matches. Migration requires the privately re-entered legacy password;
password change uses a fresh exact-operation proof of the old secure password.
Rewrap obtains a fresh proof for the same saved effect. First check authenticated
receipts/current state so already-committed or superseded effects are reconciled,
never sent through classic password routes. Preserve effect digest separately
from each durably staged authorized transport attempt; unknown outcome never
rewinds. Nested recovery sign-in must not overwrite the original pending journal.

`POST /accounts/vault/rewrap`:
`{grant,operation,signature,wrapper}`. Operation is `rewrap`; wrapper must use the
current credential generation, retained same vault key, and exact prior wrapper/
vault state. This repairs an old wrapper after email reset without changing that
new password again. It never overwrites or fabricates vault payload/identity.

## Recovery email and forgotten-password reset

Authenticated recovery-email status and email verification can keep existing v2
routes. For secure accounts, old-password enrollment and completion routes must
reject inside their commit transaction, and old profile responses report
`password_available:false`. Updated native email enrollment uses
`POST /accounts/recovery-email` with `{grant,operation,email}`; the body-bound
`set_recovery_email` operation hashes the normalized target email. Consume the
grant atomically with the mail-token reservation. No secure password goes to v2.

Existing generic email reset request remains non-enumerating. Token fingerprint
checks must support both a legacy password verifier and secure credential
generation. V2 token inspection adds `secure_reset_required:true` for secure
accounts; their browser page offers token-free `wisp://account/reset` plus explicit
paste of link/code in native Wisp, never a secure new-password field. Native input
accepts the supported HTTPS reset link/fragment or bare token. No token-bearing
custom URI, launch argument, telemetry, clipboard auto-read or history entry.

`POST /auth/reset/start` takes `{operation_id,token,generation,request}` and returns
registration started shape after validating the email token. Bind authorization
to token hash, account, current credential generation and proposed generation.
`POST /auth/reset/finish` takes
`{authorization,effect:ResetEffect,token,registration}`. ResetEffect binds its
version, scope, operation ID, expected/new credential generations and the entire
RegistrationTranscript digest. It has no original device and does not contain
an email-token digest: authorization may renew while the immutable effect stays
the same. Atomically consume token,
replace the secure credential and save an exact-effect receipt while preserving
encrypted vault/wrapper. Return `{completed:true,scope,credential_generation,backup_locked:true}`.
No new device/session is issued; never change the device's selected account.
The old vault remains locked until a trusted device rewraps its retained key.

A committed reset receipt also retains the hashes of its consumed token and
authorization. An exact finish retry with that same proof/effect may return the
original public completion result after token consumption/expiry; it does not
change credentials again or grant a session. Unknown/expired authorization cannot
read arbitrary receipts. If the original token expired before commit, a newly
issued valid reset token for the same account/current generation may authorize
the same saved effect via reset/start. Preserve original registration messages,
scope/key material and expected/new generations. If current generation changed,
do not apply the old effect: a fresh valid reset token may query
`POST /auth/reset/status` with `{token,operation_id,effect_sha256}` to obtain a
same-account public receipt/current generation for explicit reconciliation or
supersession. This endpoint issues no device/session and changes no password.
Receipt absence alone never means the old reset failed.

## Resolving interrupted operations

Exact receipt response shape:
`{operation_id,operation_sha256,scope,device_id,kind,result,committed_at}`.
`device_id` is null for email reset. `result` is the original finish response;
its credential generation and vault checkpoint are historical. Fetch current
status separately before continuing work. Vault writes return
`{committed:true,operation_id,operation_sha256,scope,credential_generation,vault}`;
rewrap returns the same fields plus `wrapper_generation`.

`GET /accounts/operations/{operation_id}` requires a session for the same account
and returns only a committed receipt: operation ID, immutable effect digest,
scope, original device (null for email reset), kind, resulting credential generation/checkpoint and
commit time. No credentials or secrets are returned. No receipt is not a
non-commit verdict. Exact commit retries return the same result after checking
the receipt's immutable binding and current caller authorization.

A prospective device may be revoked before recovery, so a failed session probe
must not trap the user forever. A fresh secure session to the same account can
verify the saved receipt or explicitly supersede a stranded operation after
verifying current account identity and vault state against the staged identity/
checkpoint. Preserve private keys throughout. A signup that never committed uses
the exact-effect renewal path above; never silently generate a replacement.
