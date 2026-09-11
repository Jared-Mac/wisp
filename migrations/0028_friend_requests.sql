CREATE TABLE friend_requests (
    sender_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    recipient_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    created_at TEXT NOT NULL,
    PRIMARY KEY (sender_id, recipient_id),
    CHECK (sender_id <> recipient_id)
);
-- Crossed requests remain a single pending invitation, requiring acceptance.
CREATE UNIQUE INDEX friend_requests_pair ON friend_requests (
    min(sender_id, recipient_id), max(sender_id, recipient_id)
);
CREATE INDEX friend_requests_recipient ON friend_requests(recipient_id);
INSERT OR IGNORE INTO schema_migrations(version, applied_at)
VALUES (28, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'));
