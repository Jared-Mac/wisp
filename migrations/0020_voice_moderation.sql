CREATE TABLE voice_moderation (
    user_id TEXT PRIMARY KEY NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    muted INTEGER NOT NULL DEFAULT 0 CHECK (muted IN (0, 1)),
    deafened INTEGER NOT NULL DEFAULT 0 CHECK (deafened IN (0, 1)),
    changed_by TEXT NOT NULL REFERENCES users(id),
    changed_at TEXT NOT NULL
);
INSERT OR IGNORE INTO schema_migrations(version, applied_at)
VALUES (20, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'));
