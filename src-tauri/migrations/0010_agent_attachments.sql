CREATE TABLE IF NOT EXISTS agent_attachments (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    session_id INTEGER NOT NULL REFERENCES agent_sessions(id) ON DELETE CASCADE,
    message_id INTEGER REFERENCES agent_messages(id) ON DELETE SET NULL,
    original_name TEXT NOT NULL,
    mime_type TEXT,
    byte_size INTEGER NOT NULL,
    storage_path TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_agent_attachments_session_id ON agent_attachments(session_id);
CREATE INDEX IF NOT EXISTS idx_agent_attachments_message_id ON agent_attachments(message_id);
