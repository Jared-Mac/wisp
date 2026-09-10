-- The dashboard increments this in the same transaction as a storage cleanup.
-- No message bodies, channel names, or administrator credentials enter events.
CREATE TABLE storage_cleanup_revision (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    revision INTEGER NOT NULL DEFAULT 0 CHECK (revision >= 0)
);
INSERT INTO storage_cleanup_revision(id, revision) VALUES (1, 0);
INSERT OR IGNORE INTO schema_migrations(version, applied_at)
VALUES (27, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'));
