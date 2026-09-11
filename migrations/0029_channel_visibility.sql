-- Existing channels retain their explicit audience; new channels default public.
ALTER TABLE server_channels ADD COLUMN visibility TEXT NOT NULL DEFAULT 'members'
    CHECK (visibility IN ('everyone','admins','members'));
CREATE TABLE channel_allowed_users (
    conversation_id TEXT NOT NULL REFERENCES server_channels(conversation_id) ON DELETE CASCADE,
    user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    PRIMARY KEY (conversation_id,user_id)
);
INSERT INTO channel_allowed_users SELECT cm.conversation_id,cm.user_id
    FROM conversation_members cm JOIN server_channels sc ON sc.conversation_id=cm.conversation_id;
CREATE VIEW channel_access AS
    SELECT sc.conversation_id,u.id user_id FROM server_channels sc CROSS JOIN users u
    WHERE u.username IS NOT NULL AND (
        sc.visibility='everyone'
        OR EXISTS(SELECT 1 FROM server_identity si WHERE si.owner_user_id=u.id)
        OR EXISTS(SELECT 1 FROM server_admins sa WHERE sa.user_id=u.id)
        OR (sc.visibility='members' AND EXISTS(SELECT 1 FROM channel_allowed_users a
            WHERE a.conversation_id=sc.conversation_id AND a.user_id=u.id))
    );
-- Keep the signed membership ledger immutable across visibility edits. Every
-- content read goes through the current access policy, including attachments.
CREATE VIEW accessible_conversation_members AS
    SELECT cm.* FROM conversation_members cm
    WHERE NOT EXISTS(SELECT 1 FROM server_channels sc WHERE sc.conversation_id=cm.conversation_id)
       OR EXISTS(SELECT 1 FROM channel_access a WHERE a.conversation_id=cm.conversation_id AND a.user_id=cm.user_id);
ALTER TABLE file_uploads ADD COLUMN recipient_ids TEXT;
INSERT OR IGNORE INTO schema_migrations(version, applied_at)
VALUES (29, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'));
