CREATE TABLE account_avatars (
    user_id TEXT PRIMARY KEY NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    png BLOB NOT NULL
);
INSERT OR IGNORE INTO schema_migrations(version, applied_at)
VALUES (25, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'));
