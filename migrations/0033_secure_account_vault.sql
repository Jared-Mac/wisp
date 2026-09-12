-- Server authentication material and opaque ciphertext only. No account private
-- identity, vault key, export key, plaintext backup or new password is stored.
CREATE TABLE secure_auth_setup (
    id INTEGER PRIMARY KEY CHECK(id=1),
    version INTEGER NOT NULL CHECK(version=1),
    material BLOB NOT NULL CHECK(length(material)>0 AND length(material)<=4096),
    digest TEXT NOT NULL UNIQUE CHECK(length(digest)=64)
);
CREATE TABLE secure_credentials (
    user_id TEXT PRIMARY KEY NOT NULL REFERENCES chat_identities(user_id),
    generation TEXT NOT NULL UNIQUE CHECK(length(generation)=36),
    password_file BLOB NOT NULL CHECK(length(password_file)>0 AND length(password_file)<=4096),
    setup_digest TEXT NOT NULL REFERENCES secure_auth_setup(digest),
    updated_at INTEGER NOT NULL
);
CREATE TRIGGER secure_credentials_require_no_legacy_password
BEFORE INSERT ON secure_credentials
WHEN EXISTS(SELECT 1 FROM users WHERE id=NEW.user_id AND password_hash IS NOT NULL)
BEGIN
    SELECT RAISE(ABORT,'secure credential cannot retain legacy password');
END;
CREATE TRIGGER secure_accounts_block_legacy_password
BEFORE UPDATE OF password_hash ON users
WHEN NEW.password_hash IS NOT NULL
    AND EXISTS(SELECT 1 FROM secure_credentials WHERE user_id=NEW.id)
BEGIN
    SELECT RAISE(ABORT,'secure account cannot enable legacy password');
END;
CREATE TABLE account_vault_manifests (
    user_id TEXT NOT NULL REFERENCES secure_credentials(user_id) ON DELETE CASCADE,
    revision INTEGER NOT NULL CHECK(revision>0),
    digest TEXT NOT NULL CHECK(length(digest)=64),
    manifest TEXT NOT NULL CHECK(length(manifest)<=16384),
    created_at INTEGER NOT NULL,
    PRIMARY KEY(user_id,revision),
    UNIQUE(user_id,revision,digest)
);
CREATE TABLE account_vaults (
    user_id TEXT PRIMARY KEY NOT NULL REFERENCES secure_credentials(user_id) ON DELETE CASCADE,
    wrapper_generation TEXT NOT NULL CHECK(length(wrapper_generation)=36),
    wrapper TEXT NOT NULL CHECK(length(wrapper)<=16384),
    revision INTEGER NOT NULL,
    digest TEXT NOT NULL,
    envelope TEXT NOT NULL CHECK(length(envelope)<=11201216),
    updated_at INTEGER NOT NULL,
    FOREIGN KEY(user_id,revision,digest) REFERENCES account_vault_manifests(user_id,revision,digest)
);
-- Pending authentication never adds rows to devices/sessions. State is private,
-- bounded and expiring; login state includes its original authenticated context.
CREATE TABLE secure_auth_attempts (
    id TEXT PRIMARY KEY NOT NULL,
    kind TEXT NOT NULL CHECK(kind IN ('login','unlock','reauth','register','migrate','password','reset')),
    user_id TEXT,
    device_id TEXT,
    username TEXT COLLATE NOCASE,
    binding_digest TEXT NOT NULL CHECK(length(binding_digest)=64),
    state BLOB NOT NULL CHECK(length(state)<=65536),
    authorization_hash TEXT,
    expires_at INTEGER NOT NULL
);
CREATE UNIQUE INDEX secure_signup_reservations ON secure_auth_attempts(username) WHERE kind='register';
CREATE INDEX secure_attempts_expiry ON secure_auth_attempts(expires_at);
CREATE TABLE secure_reauth_grants (
    token_hash TEXT PRIMARY KEY NOT NULL,
    user_id TEXT NOT NULL REFERENCES secure_credentials(user_id) ON DELETE CASCADE,
    device_id TEXT NOT NULL REFERENCES devices(id) ON DELETE CASCADE,
    generation TEXT NOT NULL,
    effect_digest TEXT NOT NULL CHECK(length(effect_digest)=64),
    expires_at INTEGER NOT NULL,
    consumed_at INTEGER
);
CREATE INDEX secure_grants_expiry ON secure_reauth_grants(expires_at);
CREATE TABLE secure_operation_receipts (
    id TEXT PRIMARY KEY NOT NULL,
    user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    device_id TEXT,
    kind TEXT NOT NULL CHECK(kind IN ('register','migrate','password','rewrap','vault','recovery_email','reset','login')),
    effect_digest TEXT NOT NULL CHECK(length(effect_digest)=64),
    result TEXT NOT NULL CHECK(length(result)<=16384),
    authorization_hash TEXT,
    reset_token_hash TEXT,
    committed_at INTEGER NOT NULL
);
CREATE INDEX secure_receipts_user ON secure_operation_receipts(user_id,committed_at);
INSERT OR IGNORE INTO schema_migrations(version,applied_at)
VALUES(33,strftime('%Y-%m-%dT%H:%M:%fZ','now'));
