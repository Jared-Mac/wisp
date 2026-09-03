CREATE TABLE file_uploads (
    id TEXT PRIMARY KEY NOT NULL,
    owner_id TEXT NOT NULL REFERENCES users(id),
    conversation_id TEXT NOT NULL REFERENCES conversations(id),
    file_name TEXT NOT NULL,
    caption TEXT NOT NULL,
    size INTEGER NOT NULL CHECK (size >= 0),
    received_bytes INTEGER NOT NULL DEFAULT 0,
    next_chunk INTEGER NOT NULL DEFAULT 0,
    keep INTEGER NOT NULL DEFAULT 0,
    last_activity_at TEXT NOT NULL,
    message_id TEXT UNIQUE REFERENCES messages(id) ON DELETE CASCADE
);
CREATE TABLE file_chunks (
    upload_id TEXT NOT NULL REFERENCES file_uploads(id) ON DELETE CASCADE,
    chunk_index INTEGER NOT NULL,
    data BLOB NOT NULL,
    PRIMARY KEY (upload_id, chunk_index)
);
ALTER TABLE chat_files ADD COLUMN upload_id TEXT REFERENCES file_uploads(id) ON DELETE CASCADE;
CREATE INDEX pending_upload_activity ON file_uploads(last_activity_at) WHERE message_id IS NULL;
CREATE INDEX file_message_expiry ON messages(json_extract(payload, '$.expires_at'))
    WHERE content_type = 'application/octet-stream';
INSERT OR IGNORE INTO schema_migrations(version, applied_at)
VALUES (8, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'));
