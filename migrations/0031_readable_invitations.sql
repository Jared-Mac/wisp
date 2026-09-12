-- Only active aliases retain labels/ciphertext. History prevents code reuse
-- without retaining expired addresses, users, or invitation payloads.
CREATE TABLE invitation_alias_history (
    lookup_id TEXT PRIMARY KEY NOT NULL
) WITHOUT ROWID;
CREATE TABLE invitation_aliases (
    invite_id TEXT PRIMARY KEY NOT NULL REFERENCES server_invites(id) ON DELETE CASCADE,
    label TEXT NOT NULL UNIQUE,
    lookup_id TEXT UNIQUE,
    envelope TEXT
);
CREATE INDEX server_invites_expiry ON server_invites(expires_at);
INSERT OR IGNORE INTO schema_migrations(version, applied_at)
VALUES (31, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'));
