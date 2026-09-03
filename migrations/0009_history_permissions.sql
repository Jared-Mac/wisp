ALTER TABLE conversation_members ADD COLUMN role TEXT NOT NULL DEFAULT 'member'
    CHECK(role IN ('member', 'host', 'admin'));
-- Preserve the existing circle administrator's authority; do not promote other members.
UPDATE conversation_members SET role = 'admin'
WHERE user_id = '00000000-0000-4000-8000-000000000001'
AND conversation_id IN (SELECT id FROM conversations WHERE kind != 'direct');
INSERT OR IGNORE INTO schema_migrations(version, applied_at)
VALUES (9, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'));
