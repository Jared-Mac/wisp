PRAGMA foreign_keys = ON;

-- A Spot survives the media room created for each visit, so its conversation
-- must reference the Spot rather than one ephemeral hangout.
ALTER TABLE conversations
    ADD COLUMN spot_id TEXT REFERENCES spots(id) ON DELETE CASCADE;

CREATE UNIQUE INDEX conversations_spot_unique
    ON conversations(spot_id) WHERE spot_id IS NOT NULL;

-- Early private-alpha builds created one Porch conversation per room visit.
-- Those histories were explicitly discarded before switching to one durable
-- conversation, so remove the legacy rows instead of merging their messages.
DELETE FROM messages
WHERE conversation_id IN (
    SELECT c.id
    FROM conversations c
    JOIN hangouts h ON h.id = c.hangout_id
    WHERE h.spot_id IS NOT NULL
);

DELETE FROM message_reads
WHERE conversation_id IN (
    SELECT c.id
    FROM conversations c
    JOIN hangouts h ON h.id = c.hangout_id
    WHERE h.spot_id IS NOT NULL
);

DELETE FROM conversation_members
WHERE conversation_id IN (
    SELECT c.id
    FROM conversations c
    JOIN hangouts h ON h.id = c.hangout_id
    WHERE h.spot_id IS NOT NULL
);

DELETE FROM conversations
WHERE hangout_id IN (
    SELECT id FROM hangouts WHERE spot_id IS NOT NULL
);

INSERT INTO conversations(id, kind, label, spot_id, created_at)
SELECT
    'spot:' || id,
    'hangout',
    name,
    id,
    strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
FROM spots;

INSERT INTO conversation_members(conversation_id, user_id, joined_at)
SELECT
    'spot:' || s.id,
    cm.user_id,
    strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
FROM spots s
CROSS JOIN circle_members cm;

INSERT OR IGNORE INTO schema_migrations(version, applied_at)
VALUES (3, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'));
