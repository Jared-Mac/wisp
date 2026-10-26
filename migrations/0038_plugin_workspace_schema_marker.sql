-- Record the workspace migration in the compatibility schema ledger as well as SQLx.
INSERT OR IGNORE INTO schema_migrations(version,applied_at)
VALUES(37,strftime('%Y-%m-%dT%H:%M:%fZ','now'));
INSERT OR IGNORE INTO schema_migrations(version,applied_at)
VALUES(38,strftime('%Y-%m-%dT%H:%M:%fZ','now'));
