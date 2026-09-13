-- Separate optional discovery resource; existing vault Bundle v1 is unchanged.
-- Only signed manifests and opaque ciphertext. Never target-service credentials.
CREATE TABLE server_catalog_manifests (
    user_id TEXT NOT NULL REFERENCES account_vaults(user_id) ON DELETE CASCADE,
    revision INTEGER NOT NULL CHECK(revision>0),
    digest TEXT NOT NULL CHECK(length(digest)=64),
    manifest TEXT NOT NULL CHECK(length(manifest)<=16384),
    created_at INTEGER NOT NULL,
    PRIMARY KEY(user_id,revision),
    UNIQUE(user_id,revision,digest)
);
CREATE TABLE server_catalogs (
    user_id TEXT PRIMARY KEY NOT NULL REFERENCES account_vaults(user_id) ON DELETE CASCADE,
    revision INTEGER NOT NULL,
    digest TEXT NOT NULL,
    envelope TEXT NOT NULL CHECK(length(envelope)<=1414508),
    updated_at INTEGER NOT NULL,
    FOREIGN KEY(user_id,revision,digest) REFERENCES server_catalog_manifests(user_id,revision,digest)
);
INSERT OR IGNORE INTO schema_migrations(version,applied_at)
VALUES(34,strftime('%Y-%m-%dT%H:%M:%fZ','now'));
