ALTER TABLE users ADD COLUMN presence TEXT NOT NULL DEFAULT 'open'
    CHECK (presence IN ('open', 'knock', 'closed', 'away'));

INSERT OR IGNORE INTO schema_migrations(version, applied_at)
VALUES (21, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'));
