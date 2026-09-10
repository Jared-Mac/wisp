# Replies, forwarding, and pins

Reply, Forward, and Pin/Unpin live in each message's three-dot menu. Replies add a
cancellable composer bar, preserve the draft on failure, and quote a compact
preview. Clicking the preview loads and highlights the original, including a
message older than the recent snapshot. Each chat header has a pins button in the
main window, tray, and detached chat windows.

Server owners and admins manage pins in server rooms and channels. Both DM
participants manage their shared pins; members of private groups manage group
pins. The server checks membership, roles, and cleared-history visibility on
reads and writes. Deleting a message removes its pin. Clearing personal history
hides old pins for that participant without changing the other person's list.

Forwarding requires choosing a destination and pressing Forward. Connected-server
chats and friends are available; choosing a friend creates a DM only after that
confirmation. A forward contains the selected message body and original quoted
sender name. It deliberately omits the source chat identifier and any original
reply context. Files and images are copied and encrypted for the destination's
verified recipients with fresh keys, so source-chat access is unnecessary.

Reply previews and forwarding attribution are sender-provided quotations, not
proof that the quoted author signed the quote. In encrypted chats, they are
inside the sender-authenticated ciphertext. Pin records contain message/chat
IDs, actor, and timestamp; the server never needs the decrypted message body.
Migration 26 adds legacy message-context storage and pins.

Recipients need an updated client to read encrypted replies and forwards. Older
clients reject the new encrypted context field; ordinary messages still omit it
and retain their existing format. The feature does not change the default theme
or the user's saved appearance.

Validation: `cargo test --workspace --locked`, `cargo clippy --workspace
--all-targets --locked -- -D warnings`, and `bash scripts/test-message-actions.sh`.
The daemon integration test uses separate loopback servers and isolated encryption
keys; it never joins voice or accesses production messages.
