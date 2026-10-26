-- Internal encrypted channels back server-wide plugin packs, not chat navigation.
CREATE TABLE plugin_workspaces (
    plugin_id TEXT PRIMARY KEY,
    conversation_id TEXT NOT NULL UNIQUE REFERENCES conversations(id)
);
