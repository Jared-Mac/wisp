CREATE TABLE room_invitations (
    id TEXT PRIMARY KEY,
    message_id TEXT NOT NULL REFERENCES messages(id) ON DELETE CASCADE,
    hangout_id TEXT NOT NULL REFERENCES hangouts(id) ON DELETE CASCADE,
    sender_id TEXT NOT NULL REFERENCES users(id),
    recipient_id TEXT NOT NULL REFERENCES users(id),
    created_at TEXT NOT NULL,
    expires_at TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'pending' CHECK(status IN ('pending', 'accepted', 'dismissed'))
);
CREATE UNIQUE INDEX room_invitations_pending ON room_invitations(hangout_id, recipient_id) WHERE status = 'pending';
CREATE INDEX room_invitations_recipient ON room_invitations(recipient_id, status, expires_at);
