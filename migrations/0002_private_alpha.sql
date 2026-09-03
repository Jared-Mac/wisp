PRAGMA foreign_keys = ON;

CREATE TABLE conversations (
    id TEXT PRIMARY KEY NOT NULL,
    kind TEXT NOT NULL CHECK (kind IN ('direct', 'circle', 'hangout')),
    label TEXT NOT NULL,
    circle_id TEXT REFERENCES circles(id) ON DELETE CASCADE,
    hangout_id TEXT REFERENCES hangouts(id) ON DELETE CASCADE,
    created_at TEXT NOT NULL
);

CREATE UNIQUE INDEX conversations_circle_unique
    ON conversations(circle_id) WHERE circle_id IS NOT NULL;
CREATE UNIQUE INDEX conversations_hangout_unique
    ON conversations(hangout_id) WHERE hangout_id IS NOT NULL;

CREATE TABLE conversation_members (
    conversation_id TEXT NOT NULL REFERENCES conversations(id) ON DELETE CASCADE,
    user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    joined_at TEXT NOT NULL,
    PRIMARY KEY (conversation_id, user_id)
);

CREATE INDEX conversation_members_user
    ON conversation_members(user_id, conversation_id);

CREATE TABLE message_reads (
    conversation_id TEXT NOT NULL REFERENCES conversations(id) ON DELETE CASCADE,
    user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    last_read_at TEXT NOT NULL,
    PRIMARY KEY (conversation_id, user_id)
);

CREATE TABLE spots (
    id TEXT PRIMARY KEY NOT NULL,
    name TEXT NOT NULL UNIQUE COLLATE NOCASE,
    created_at TEXT NOT NULL
);

ALTER TABLE hangouts ADD COLUMN spot_id TEXT REFERENCES spots(id);
CREATE INDEX hangouts_active_spot ON hangouts(spot_id, ended_at);

CREATE TABLE device_invites (
    id TEXT PRIMARY KEY NOT NULL,
    code_hash TEXT NOT NULL UNIQUE,
    user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    created_by TEXT NOT NULL REFERENCES users(id),
    created_at TEXT NOT NULL,
    expires_at TEXT NOT NULL,
    used_at TEXT
);

CREATE TABLE devices (
    id TEXT PRIMARY KEY NOT NULL,
    user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    token_hash TEXT NOT NULL UNIQUE,
    created_at TEXT NOT NULL,
    last_seen_at TEXT,
    revoked_at TEXT
);

CREATE INDEX devices_user ON devices(user_id, revoked_at);

CREATE TABLE sessions (
    id TEXT PRIMARY KEY NOT NULL,
    device_id TEXT NOT NULL REFERENCES devices(id) ON DELETE CASCADE,
    token_hash TEXT NOT NULL UNIQUE,
    created_at TEXT NOT NULL,
    expires_at TEXT NOT NULL,
    revoked_at TEXT
);

CREATE INDEX sessions_token ON sessions(token_hash, expires_at, revoked_at);

INSERT INTO conversations(id, kind, label, circle_id, created_at)
SELECT
    '00000000-0000-4000-8000-000000000011',
    'circle',
    'Friends',
    '00000000-0000-4000-8000-000000000010',
    strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
WHERE EXISTS (
    SELECT 1 FROM circles
    WHERE id = '00000000-0000-4000-8000-000000000010'
);

INSERT INTO conversation_members(conversation_id, user_id, joined_at)
SELECT
    '00000000-0000-4000-8000-000000000011',
    user_id,
    strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
FROM circle_members
WHERE circle_id = '00000000-0000-4000-8000-000000000010';

INSERT INTO spots(id, name, created_at)
VALUES (
    '00000000-0000-4000-8000-000000000020',
    'Porch',
    strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
);

INSERT OR IGNORE INTO schema_migrations(version, applied_at)
VALUES (2, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'));
