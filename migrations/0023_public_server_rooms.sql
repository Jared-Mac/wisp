-- Previously every user-created room was implicitly private; there was no
-- administrator setting for it. Ordinary rooms on account-based servers now
-- default to server-wide discovery. Explicit privacy choices start here.
UPDATE spots SET private = 0 WHERE EXISTS (SELECT 1 FROM server_identity);
ALTER TABLE pending_room_admissions ADD COLUMN automatic INTEGER NOT NULL DEFAULT 0;

INSERT OR IGNORE INTO schema_migrations(version, applied_at)
VALUES (23, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'));
