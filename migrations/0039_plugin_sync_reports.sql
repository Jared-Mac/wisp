-- Device-bound, advisory installation reports. Never store paths or mod contents.
CREATE TABLE plugin_sync_reports (
    plugin_id TEXT NOT NULL REFERENCES server_plugins(plugin_id) ON DELETE CASCADE,
    user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    device_id TEXT NOT NULL,
    revision TEXT NOT NULL REFERENCES messages(id) ON DELETE CASCADE,
    status TEXT NOT NULL CHECK(status IN ('up_to_date','update_available','modified','missing','not_installed','unknown')),
    platform TEXT NOT NULL CHECK(platform IN ('linux','windows','macos','android','other')),
    checked_at INTEGER NOT NULL,
    reported_at INTEGER NOT NULL,
    PRIMARY KEY(plugin_id,user_id,device_id)
);
CREATE TRIGGER clear_plugin_sync_on_server_leave
AFTER UPDATE OF server_member ON users WHEN NEW.server_member=0
BEGIN
    DELETE FROM plugin_sync_reports WHERE user_id=NEW.id;
END;
INSERT INTO schema_migrations(version,applied_at)
VALUES(39,strftime('%Y-%m-%dT%H:%M:%fZ','now'));
