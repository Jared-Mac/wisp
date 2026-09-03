ALTER TABLE spots ADD COLUMN private INTEGER NOT NULL DEFAULT 0;
UPDATE conversation_members SET role = 'host'
WHERE conversation_id = 'spot:00000000-0000-4000-8000-000000000020'
AND user_id = '00000000-0000-4000-8000-000000000001';
INSERT OR IGNORE INTO schema_migrations(version, applied_at)
VALUES (10, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'));
