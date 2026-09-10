CREATE TABLE soundboard_sounds (
    id TEXT PRIMARY KEY NOT NULL,
    owner_id TEXT NOT NULL REFERENCES users(id),
    name TEXT NOT NULL COLLATE NOCASE UNIQUE,
    wav BLOB NOT NULL,
    duration_ms INTEGER NOT NULL CHECK(duration_ms BETWEEN 1 AND 10000),
    created_at TEXT NOT NULL
);
INSERT OR IGNORE INTO schema_migrations(version, applied_at)
VALUES (24, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'));
