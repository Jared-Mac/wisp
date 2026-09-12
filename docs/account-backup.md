# Encrypted account backup

Account backup lets you sign in on another desktop or Android device and restore
your encryption identity, media key, and saved trust settings without moving a
recovery file between devices. The service stores an encrypted backup; your
secure password and decrypted keys stay in the native client.

## Existing accounts

On a device that already has your encryption keys, open **Settings → Profile →
Account backup**. Enter your current password and choose a new, different secure
password. Enabling backup preserves the account, friendships, public identity,
and encryption keys. Older clients that only support classic password sign-in
must be updated before signing in again.

New accounts use secure sign-in and encrypted backup automatically. Accounts can
be created and used before joining a server. Your sign-in username is separate
from your public username and display name.

## Another device

Use secure sign-in with the same account service, sign-in username, and password.
The client restores the existing keys. It never silently creates replacements
for an account with a locked or missing backup. Signing in does not join voice,
start a camera, or share a screen.

Sync runs about once per minute by default. **Sync now** and **Sync automatically**
are in Account backup settings. An unchanged check downloads only small status
metadata. Failed or interrupted actions keep their recovery state and can be
resumed from the same section or the sign-in window.

## Changing or resetting a password

Changing your password in Profile settings keeps the same backup key and updates
how it is unlocked. Verify a recovery email before you need a password reset.

An email reset restores account sign-in, but it cannot by itself decrypt the old
backup. Paste the email reset link into Wisp's native reset form and choose a new
password. Then, on an existing trusted device, use **Restore backup access** with
the new password. Other devices can unlock the repaired backup afterward.

If you lose every trusted device and forget the secure password, an email reset
cannot recover the encrypted history. Keep a private recovery copy and trusted
devices available. A classic identity-only recovery file does not contain every
saved trust setting or media key in an account backup.

This feature uses OPAQUE password authentication and authenticated encryption.
Wisp's complete integration has not had an independent security audit; the
protocol details and tested limits are in `design/account-vault-v1.md`.
