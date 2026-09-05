-- Earlier private-alpha migrations created a fixed development topology before
-- runtime configuration was known. Remove only its exact empty fixture rows.
-- Any installation that used them (messages, devices, or a real account) is
-- preserved; this is intentionally narrower than deleting unnamed accounts.
DELETE FROM conversations
WHERE id IN (
    '00000000-0000-4000-8000-000000000011',
    'spot:00000000-0000-4000-8000-000000000020'
)
AND NOT EXISTS (
    SELECT 1 FROM messages WHERE messages.conversation_id = conversations.id
)
AND NOT EXISTS (
    SELECT 1
    FROM conversation_members
    JOIN users ON users.id = conversation_members.user_id
    WHERE conversation_members.conversation_id = conversations.id
      AND users.username IS NOT NULL
);

DELETE FROM spots
WHERE id = '00000000-0000-4000-8000-000000000020'
  AND NOT EXISTS (SELECT 1 FROM conversations WHERE conversations.spot_id = spots.id)
  AND NOT EXISTS (SELECT 1 FROM hangouts WHERE hangouts.spot_id = spots.id);

DELETE FROM circles
WHERE id = '00000000-0000-4000-8000-000000000010'
  AND NOT EXISTS (SELECT 1 FROM conversations WHERE conversations.circle_id = circles.id);

DELETE FROM users
WHERE id IN (
    '00000000-0000-4000-8000-000000000001',
    '00000000-0000-4000-8000-000000000002',
    '00000000-0000-4000-8000-000000000003',
    '00000000-0000-4000-8000-000000000004'
)
  AND username IS NULL
  AND NOT EXISTS (SELECT 1 FROM devices WHERE devices.user_id = users.id)
  AND NOT EXISTS (SELECT 1 FROM messages WHERE messages.sender_id = users.id);

INSERT OR IGNORE INTO schema_migrations(version, applied_at)
VALUES (15, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'));
