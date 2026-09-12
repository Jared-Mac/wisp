-- Existing installations keep their accounts, permissions and encryption IDs.
-- Only the new public registration path creates an account without membership.
ALTER TABLE users ADD COLUMN server_member INTEGER NOT NULL DEFAULT 1 CHECK(server_member IN (0,1));
ALTER TABLE users ADD COLUMN public_handle TEXT COLLATE NOCASE;
CREATE UNIQUE INDEX users_public_handle ON users(public_handle) WHERE public_handle IS NOT NULL;
CREATE TABLE account_blocks (
    user_id TEXT NOT NULL REFERENCES users(id),
    blocked_id TEXT NOT NULL REFERENCES users(id),
    PRIMARY KEY(user_id,blocked_id), CHECK(user_id<>blocked_id)
);
CREATE TABLE server_invites (
    id TEXT PRIMARY KEY,
    code_hash TEXT NOT NULL UNIQUE,
    created_by TEXT NOT NULL REFERENCES users(id),
    created_at TEXT NOT NULL,
    expires_at TEXT NOT NULL,
    used_at TEXT,
    used_by TEXT REFERENCES users(id),
    revoked_at TEXT,
    lookup_id TEXT UNIQUE,
    envelope TEXT
);
DROP VIEW accessible_conversation_members;
DROP VIEW channel_access;
CREATE VIEW channel_access AS
    SELECT sc.conversation_id,u.id user_id FROM server_channels sc CROSS JOIN users u
    WHERE u.server_member=1 AND u.username IS NOT NULL AND (
        sc.visibility='everyone'
        OR EXISTS(SELECT 1 FROM server_identity si WHERE si.owner_user_id=u.id)
        OR EXISTS(SELECT 1 FROM server_admins sa WHERE sa.user_id=u.id)
        OR (sc.visibility='members' AND EXISTS(SELECT 1 FROM channel_allowed_users a
            WHERE a.conversation_id=sc.conversation_id AND a.user_id=u.id))
    );
CREATE VIEW accessible_conversation_members AS
    SELECT cm.* FROM conversation_members cm JOIN conversations c ON c.id=cm.conversation_id
    JOIN users u ON u.id=cm.user_id
    WHERE (u.server_member=1 OR (c.kind<>'hangout' AND NOT EXISTS(
        SELECT 1 FROM server_channels sc WHERE sc.conversation_id=c.id)))
    AND (NOT EXISTS(SELECT 1 FROM server_channels sc WHERE sc.conversation_id=c.id)
       OR EXISTS(SELECT 1 FROM channel_access a WHERE a.conversation_id=c.id AND a.user_id=cm.user_id));
INSERT OR IGNORE INTO schema_migrations(version,applied_at)
VALUES(30,strftime('%Y-%m-%dT%H:%M:%fZ','now'));
