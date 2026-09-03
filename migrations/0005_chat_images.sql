CREATE TABLE chat_images (
    message_id TEXT PRIMARY KEY NOT NULL REFERENCES messages(id) ON DELETE CASCADE,
    png BLOB NOT NULL
);
INSERT OR IGNORE INTO schema_migrations(version, applied_at)
VALUES (5, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'));
