-- Preferences and history visibility belong to each participant, not the chat.
ALTER TABLE conversation_members ADD COLUMN tab_closed INTEGER NOT NULL DEFAULT 0;
ALTER TABLE conversation_members ADD COLUMN history_cleared_at TEXT;

INSERT OR IGNORE INTO schema_migrations(version, applied_at)
VALUES (4, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'));
