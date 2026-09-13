# Encrypted saved-server catalog v1

The catalog lets another signed-in device discover services already used on a
desktop. It is discovery metadata, not membership or authentication. Every
service keeps its own network/account identity and device credentials. A client
must sign in independently at a discovered origin unless it already has a
credential explicitly bound to that exact origin, network and account.

The authoritative wire and crypto types live in
`wisp_crypto::account_vault::catalog`. Existing strict vault Bundle v1,
authentication info and native account records are unchanged. Older clients can
continue using their backup without reading or erasing the separate catalog.

## Authenticated API

All paths are on the account's exact canonical service origin, use its existing
session bearer, reject redirects, and inherit v3 no-store response headers.

| Method and path | Request | Response |
| --- | --- | --- |
| GET `/v3/accounts/server-catalog/status` | — | `{version:1, scope, catalog:Checkpoint|null}` |
| GET `/v3/accounts/server-catalog` | — | `{version:1, scope, catalog:Envelope|null}` |
| POST `/v3/accounts/server-catalog` | `{catalog:Envelope}` | Status above |
| POST `/v3/accounts/server-catalog/proof` | `{after:Checkpoint, through:Checkpoint}` | `{manifests, next:Checkpoint, complete:bool}` |

An absent route (HTTP 404) means unsupported. A supported account with no catalog
returns null. Once a client retains a checkpoint, a null or older head must never
erase it. The account needs an existing secure vault; unrelated accounts and
revoked device sessions cannot read or write its catalog.

A write is an atomic compare-and-swap using the signed header's parent
checkpoint. An exact retry of the current signed envelope succeeds without a
new revision. A different parent returns HTTP 409 `catalog_state_changed`; the
client must fetch, verify and merge. No blind overwrite or reset endpoint exists.
The service stores only ciphertext and signed manifests in migration 0034's
`server_catalogs` and `server_catalog_manifests` tables.

## Plaintext and merge

`Catalog {format:1, scope:Scope, entries:Vec<Entry>}`, where
`Entry {scope:Scope, label:String}` and Scope is `{origin, network, account}`.
Both scopes use canonical origins accepted by the existing account protocol:
HTTPS, with loopback HTTP for tests/development, no path/query/fragment/userinfo.
The outer scope binds the account whose vault unlocks this catalog. Each entry
names a target service and its independently authenticated account/network.

Limits: 4096 entries; 1 MiB encoded plaintext; labels 1–160 UTF-8 bytes, trimmed,
without control characters. Entries are strictly sorted by unique origin.
Unknown/duplicate JSON fields and invalid or ambiguous origins are rejected.
No passwords, session/device tokens, recovery secrets or media keys are allowed.

V1 merges by union. Existing labels win, and an existing origin with a different
account/network causes a conflict, preserving both the current catalog and
private local state. It never silently replaces an account binding. Local
nicknames, ordering, hiding and removal from a selector stay local; synchronized
removals/tombstones are outside v1. Labels are advisory; authenticate the service
identity and obtain its current metadata before treating a card as connected.

## Cryptography and history

The existing stable VaultKey is input to the vault HKDF-SHA256 helper with salt
`wisp-account-vault-hkdf-v1` and JSON info
`["wisp-server-catalog-payload-v1", 1, scope]`. Thus catalog encryption is separate
from the account payload and password wrappers, and changing a password does not
change this encryption key.

AES-256-GCM uses a random 12-byte nonce. AAD is JSON
`["wisp-server-catalog-payload-v1", header]`. Header has the existing field layout:
`format, scope, revision, parent, signer, nonce`. A separate signed manifest
contains that header, the SHA-256 of the ciphertext, and the account identity's
signature over the existing sign-statement primitive with domain
`wisp-server-catalog-manifest-v1` and JSON statement
`["wisp-server-catalog-manifest-v1", header, ciphertext_sha256]`.

A checkpoint is the revision plus SHA-256 of that statement. The initial
revision is 1 with a null parent; successors name the exact previous checkpoint.
Existing vault signatures cannot authenticate catalogs. Header scope and signer
must match the restored account. Use the shared Rust implementation on both
platforms rather than reimplementing JSON/signature canonicalization.

Returning devices verify every signed connecting manifest from their retained
checkpoint. Proof pages contain at most 128 manifests and 512 KiB of encoded
manifests. Verify each parent/revision, the response's next/complete fields and
the final target before decrypting/installing it. Persist the exact target and
verified progress between bounded catch-up passes. A fresh device verifies the
authenticated head against its restored identity/key and pins it. As with a
fresh vault restore, it cannot independently detect server withholding of
history it has never observed; retained checkpoints reject subsequent rollback
and forks. The protocol does not claim a global transparency log.

## Desktop lifecycle

The existing minute-based account backup loop publishes member-service entries
whose scopes have been verified in local private account storage. Each unlocked
secure account syncs its own encrypted catalog. Disabled automatic backup also
disables automatic catalog sync; explicit **Sync now** includes it. Unchanged
catalog checks fetch small status metadata, not ciphertext. Transient conflicts
retry at most three times per pass; history catch-up uses at most 32 pages or
30 seconds per pass, persisting progress for the next pass.

The strict existing account journal remains untouched. A private, atomically
written `.catalog` sidecar retains the signed head, exact pending encrypted
write, and catch-up progress. An initialization marker detects later missing
history. No sync path creates a device credential, modifies the server registry,
joins voice, or starts media. Restored Android cards likewise require explicit
selection and target-specific sign-in before use.
