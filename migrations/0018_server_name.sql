ALTER TABLE server_identity
    ADD COLUMN name TEXT NOT NULL DEFAULT 'Wisp server';

INSERT OR IGNORE INTO schema_migrations(version, applied_at)
VALUES (18, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'));
