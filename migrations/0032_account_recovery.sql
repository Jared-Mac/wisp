-- Only verified addresses can recover an account. Pending changes never replace
-- the current address until the owner follows the verification link.
CREATE TABLE account_recovery_emails (
    user_id TEXT PRIMARY KEY NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    email TEXT UNIQUE,
    pending_email TEXT,
    verification_sent_at INTEGER NOT NULL DEFAULT 0,
    reset_sent_at INTEGER NOT NULL DEFAULT 0
);
CREATE TABLE account_recovery_tokens (
    token_hash TEXT PRIMARY KEY NOT NULL,
    user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    kind TEXT NOT NULL CHECK(kind IN ('verify', 'reset')),
    email TEXT NOT NULL,
    password_fingerprint TEXT NOT NULL,
    expires_at INTEGER NOT NULL,
    consumed_at INTEGER
);
CREATE INDEX account_recovery_tokens_expiry ON account_recovery_tokens(expires_at);
CREATE INDEX account_recovery_tokens_user ON account_recovery_tokens(user_id);
INSERT OR IGNORE INTO schema_migrations(version, applied_at)
VALUES (32, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'));
