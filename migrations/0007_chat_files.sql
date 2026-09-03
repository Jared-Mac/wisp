CREATE TABLE chat_files (
    message_id TEXT PRIMARY KEY NOT NULL REFERENCES messages(id) ON DELETE CASCADE,
    data BLOB NOT NULL
);
INSERT OR IGNORE INTO schema_migrations(version, applied_at)
VALUES (7, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'));
