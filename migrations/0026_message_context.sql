ALTER TABLE messages ADD COLUMN context TEXT;
ALTER TABLE file_uploads ADD COLUMN context TEXT;
CREATE TABLE message_pins (
    message_id TEXT PRIMARY KEY REFERENCES messages(id) ON DELETE CASCADE,
    conversation_id TEXT NOT NULL REFERENCES conversations(id) ON DELETE CASCADE,
    pinned_by TEXT NOT NULL REFERENCES users(id),
    pinned_at TEXT NOT NULL
);
CREATE INDEX message_pins_conversation ON message_pins(conversation_id, pinned_at);
INSERT INTO schema_migrations(version, applied_at)
VALUES (26, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'));
