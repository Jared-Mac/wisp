CREATE TABLE server_bans (
    user_id TEXT PRIMARY KEY REFERENCES users(id),
    banned_by TEXT NOT NULL REFERENCES users(id),
    banned_at TEXT NOT NULL,
    reason TEXT NOT NULL DEFAULT '' CHECK(length(reason)<=280)
);

-- Persist disconnect work so a media outage cannot silently undo moderation.
CREATE TABLE server_member_disconnects (
    user_id TEXT NOT NULL REFERENCES users(id),
    room TEXT NOT NULL,
    PRIMARY KEY(user_id,room)
);

-- All admission paths, including older account-invite clients, must respect bans.
CREATE TRIGGER server_ban_blocks_membership
BEFORE UPDATE OF server_member ON users
WHEN NEW.server_member=1 AND EXISTS(SELECT 1 FROM server_bans WHERE user_id=NEW.id)
BEGIN
    SELECT RAISE(ABORT,'server_banned');
END;

INSERT OR IGNORE INTO schema_migrations(version,applied_at)
VALUES(35,strftime('%Y-%m-%dT%H:%M:%fZ','now'));
