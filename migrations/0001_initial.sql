PRAGMA foreign_keys = ON;

CREATE TABLE IF NOT EXISTS schema_migrations (
    version INTEGER PRIMARY KEY,
    applied_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS users (
    id TEXT PRIMARY KEY NOT NULL,
    display_name TEXT NOT NULL UNIQUE COLLATE NOCASE,
    last_seen_at TEXT
);

CREATE TABLE IF NOT EXISTS circles (
    id TEXT PRIMARY KEY NOT NULL,
    name TEXT NOT NULL UNIQUE
);

CREATE TABLE IF NOT EXISTS circle_members (
    circle_id TEXT NOT NULL REFERENCES circles(id) ON DELETE CASCADE,
    user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    PRIMARY KEY (circle_id, user_id)
);

CREATE TABLE IF NOT EXISTS hangouts (
    id TEXT PRIMARY KEY NOT NULL,
    livekit_room TEXT NOT NULL UNIQUE,
    label TEXT,
    access_state TEXT NOT NULL DEFAULT 'open',
    created_at TEXT NOT NULL,
    ended_at TEXT
);

CREATE TABLE IF NOT EXISTS hangout_members (
    hangout_id TEXT NOT NULL REFERENCES hangouts(id) ON DELETE CASCADE,
    user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    joined_at TEXT NOT NULL,
    left_at TEXT,
    PRIMARY KEY (hangout_id, user_id, joined_at)
);

CREATE INDEX IF NOT EXISTS hangout_members_active
    ON hangout_members(user_id, left_at);

CREATE TABLE IF NOT EXISTS messages (
    id TEXT PRIMARY KEY NOT NULL,
    conversation_id TEXT NOT NULL,
    sender_id TEXT NOT NULL REFERENCES users(id),
    created_at TEXT NOT NULL,
    content_type TEXT NOT NULL,
    payload TEXT NOT NULL,
    encryption_version INTEGER NOT NULL DEFAULT 0
);

CREATE INDEX IF NOT EXISTS messages_conversation_created
    ON messages(conversation_id, created_at, id);

INSERT OR IGNORE INTO schema_migrations(version, applied_at)
VALUES (1, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'));
