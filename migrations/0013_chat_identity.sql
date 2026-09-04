CREATE TABLE chat_network (id INTEGER PRIMARY KEY CHECK(id=1), network_id TEXT NOT NULL);
CREATE TABLE chat_identities (user_id TEXT PRIMARY KEY REFERENCES users(id), public_identity TEXT NOT NULL);
CREATE TABLE chat_rosters (
    conversation_id TEXT NOT NULL REFERENCES conversations(id) ON DELETE CASCADE,
    revision INTEGER NOT NULL CHECK(revision>=0),
    signed_roster TEXT NOT NULL,
    PRIMARY KEY(conversation_id,revision)
);
ALTER TABLE room_invitations ADD COLUMN encrypted_membership TEXT;
