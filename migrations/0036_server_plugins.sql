-- Only reviewed catalog packages may be enabled. Private pack contents stay encrypted in chat.
CREATE TABLE server_plugins (
    plugin_id TEXT PRIMARY KEY,
    digest TEXT NOT NULL CHECK(length(digest)=64),
    enabled_by TEXT NOT NULL REFERENCES users(id),
    enabled_at TEXT NOT NULL
);
INSERT OR IGNORE INTO schema_migrations(version,applied_at)
VALUES(36,strftime('%Y-%m-%dT%H:%M:%fZ','now'));
