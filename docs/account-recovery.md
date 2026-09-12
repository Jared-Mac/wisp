# Account recovery

Add an email in **Settings → Profile → Recovery email**, enter your current
password, and follow the verification link from `support@wisp.you`. Existing
accounts do not acquire an email automatically. Newsletter addresses are separate.
Use **Forgot password?** on the sign-in screen to request a reset link with your
sign-in username or verified email. Links also work in a phone browser.

Verification links last 30 minutes; reset links last 20 minutes. Each works once.
Changing a password invalidates previously issued links. Changing a recovery
address preserves the old verified address until the new one is verified.
Resends have a one-minute cooldown, with request and global abuse limits.
Public reset requests return the same response for unknown/unverified accounts.

A reset preserves the account UUID, friendships, memberships, chat identities,
history and device credentials. Other devices remain signed in; revoke unwanted
devices separately in **Settings → Devices**. Email recovery cannot restore a
missing encryption key. Retain the encryption recovery file or an existing device.
If no email was verified before the password was lost, this flow cannot recover
that account. Enrollment requires the existing password.

Accounts with encrypted backup reset their secure password in the native Wisp
client. The browser offers a token-free **Open Wisp** link; paste the email link
into the native reset form when asked. Resetting sign-in preserves the encrypted
backup, but unlocking it afterward requires an existing trusted device to
restore backup access. See [Encrypted account backup](account-backup.md).

## Hosting

Migration 32 adds recovery addresses and hashed, expiring single-use tokens.
No plaintext link token is retained by the application database. Password changes
and token consumption are transactional, including concurrent requests. Links
carry secrets in a URL fragment; browser forms immediately remove it from history.
Never enable request-body, SMTP body, or browser analytics logging on these routes.

The owner deployment enables `WISP_RECOVERY_MAIL=true`. This explicitly selects
the existing same-host Postfix service at `127.0.0.1:25`, with fixed envelope and
header sender `support@wisp.you`. Rspamd signs mail; the application receives no
SMTP credential or DKIM private key. The transport is loopback-only. Other hosts
leave this option off until they provide their own supported delivery setup.
Core startup remains available if the local mail service is temporarily down.
SMTP acceptance means queued, not confirmed inbox delivery. There is no automatic
application retry after uncertain acceptance; the MTA handles queued delivery.
Authenticated enrollment reports delivery failures. Public requests retain their
generic response and only record a sanitized mail-failure log entry.

`infra/account-recovery/site/` is served on `wisp.you` using the accompanying
Caddy snippet. These forms use the existing same-origin `/v2/` reverse proxy.
Serve only its three named pages and two assets; preserve existing invite, Android,
website and API routes. Use no-store, noindex, no-referrer and the supplied CSP.
Android App Links exclude `/account/`; token links open in the browser.

## API

- `GET /v2/accounts/recovery-email` (device session): verified/pending address and
  whether mail delivery is configured.
- `POST /v2/accounts/recovery-email` (classic device session): `email`, `current_password`.
- `POST /v2/accounts/recovery-email/verify`: `token`.
- `POST /v2/accounts/password-reset/request`: `identifier`; always generic `202`
  after request-wide capacity/throttling checks when delivery is configured.
- `POST /v2/accounts/password-reset/inspect`: `token`; validity/expiry and whether
  native secure reset is required.
- `POST /v2/accounts/password-reset/complete`: classic accounts only; `token`,
  `new_password`.

Migration 33 adds secure credentials and encrypted account backup. Secure
accounts use native `/v3/auth/reset/{start,finish,status}` and exact-operation
reauthentication for `/v3/accounts/recovery-email`. Their secure passwords never
go to the classic endpoints. Interrupted native resets retain a private local
journal for receipt lookup or exact retry. See the
[native protocol](design/account-vault-api-v1.md) for the complete contract.

These routes do not require membership in a server. Error messages are chosen
from known protocol codes by both clients; arbitrary server bodies are not shown.

Validation covers authentication, email verification, expiry, cross-purpose
tokens, single use, concurrent consumption, password-change invalidation, generic
responses, rate limits, device preservation and local SMTP envelope/body handling.
No real user's password, devices, recovery address or email is changed by tests.
