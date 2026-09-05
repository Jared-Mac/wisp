CREATE TABLE custom_emojis (
    id TEXT PRIMARY KEY NOT NULL,
    owner_id TEXT NOT NULL REFERENCES users(id),
    scope TEXT NOT NULL CHECK (scope IN ('account', 'server')),
    name TEXT NOT NULL,
    png BLOB NOT NULL,
    created_at TEXT NOT NULL,
    removed_at TEXT
);
CREATE UNIQUE INDEX custom_emoji_account_name ON custom_emojis(owner_id,name)
    WHERE scope='account' AND removed_at IS NULL;
CREATE UNIQUE INDEX custom_emoji_server_name ON custom_emojis(name)
    WHERE scope='server' AND removed_at IS NULL;
CREATE TABLE message_reactions (
    id TEXT PRIMARY KEY NOT NULL,
    target_id TEXT NOT NULL REFERENCES messages(id) ON DELETE CASCADE,
    sender_id TEXT NOT NULL REFERENCES users(id),
    created_at TEXT NOT NULL,
    content_type TEXT NOT NULL,
    payload TEXT NOT NULL,
    encryption_version INTEGER NOT NULL CHECK (encryption_version IN (0,1))
);
CREATE INDEX message_reactions_target ON message_reactions(target_id);
INSERT OR IGNORE INTO schema_migrations(version, applied_at)
VALUES (22, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'));
