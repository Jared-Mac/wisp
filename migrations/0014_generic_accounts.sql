-- Public-server identity. Accounts are data created through bootstrap or an
-- invitation; no person or room is compiled into Wisp.
ALTER TABLE users ADD COLUMN username TEXT;
ALTER TABLE users ADD COLUMN password_hash TEXT;
ALTER TABLE users ADD COLUMN created_at TEXT;

CREATE UNIQUE INDEX users_username_unique
    ON users(username COLLATE NOCASE)
    WHERE username IS NOT NULL;

CREATE TABLE server_identity (
    id INTEGER PRIMARY KEY CHECK(id = 1),
    owner_user_id TEXT NOT NULL REFERENCES users(id)
);

CREATE TABLE friendships (
    first_user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    second_user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    created_at TEXT NOT NULL,
    CHECK(first_user_id < second_user_id),
    PRIMARY KEY(first_user_id, second_user_id)
);

CREATE TABLE account_invites (
    id TEXT PRIMARY KEY NOT NULL,
    code_hash TEXT NOT NULL UNIQUE,
    created_by TEXT NOT NULL REFERENCES users(id),
    kind TEXT NOT NULL CHECK(kind IN ('friend', 'room')),
    conversation_id TEXT REFERENCES conversations(id) ON DELETE CASCADE,
    created_at TEXT NOT NULL,
    expires_at TEXT NOT NULL,
    used_at TEXT,
    used_by TEXT REFERENCES users(id),
    CHECK((kind = 'friend' AND conversation_id IS NULL)
       OR (kind = 'room' AND conversation_id IS NOT NULL))
);

-- Joining an already-encrypted room requires an owner/admin client signature.
-- Account registration records the accepted invitation here; an authorized
-- client consumes it by publishing the next signed room roster.
CREATE TABLE pending_room_admissions (
    conversation_id TEXT NOT NULL REFERENCES conversations(id) ON DELETE CASCADE,
    user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    invited_by TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    created_at TEXT NOT NULL,
    PRIMARY KEY(conversation_id, user_id)
);

-- A brand-new database passes through the private-alpha migrations before
-- reaching this one. Remove their placeholder topology only when no account or
-- device has ever been created. Existing installations retain all data.
DELETE FROM conversations WHERE NOT EXISTS (SELECT 1 FROM users);
DELETE FROM spots WHERE NOT EXISTS (SELECT 1 FROM users);
DELETE FROM circles WHERE NOT EXISTS (SELECT 1 FROM users);

INSERT OR IGNORE INTO schema_migrations(version, applied_at)
VALUES (14, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'));
