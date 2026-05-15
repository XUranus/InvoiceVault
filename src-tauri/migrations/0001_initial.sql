-- raw_files
CREATE TABLE IF NOT EXISTS raw_files (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    sha256 TEXT NOT NULL UNIQUE,
    md5 TEXT,
    original_name TEXT NOT NULL,
    current_name TEXT,
    extension TEXT NOT NULL,
    mime_type TEXT,
    byte_size INTEGER NOT NULL,
    storage_path TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

-- import_jobs
CREATE TABLE IF NOT EXISTS import_jobs (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    raw_file_id INTEGER,
    source_path TEXT NOT NULL,
    status TEXT NOT NULL,
    error_message TEXT,
    source_type TEXT NOT NULL DEFAULT 'manual',
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (raw_file_id) REFERENCES raw_files(id)
);

CREATE INDEX IF NOT EXISTS idx_import_jobs_status ON import_jobs(status);
CREATE INDEX IF NOT EXISTS idx_import_jobs_created_at ON import_jobs(created_at);

-- invoices
CREATE TABLE IF NOT EXISTS invoices (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    raw_file_id INTEGER NOT NULL,
    invoice_type TEXT,
    invoice_code TEXT,
    invoice_number TEXT,
    issue_date TEXT,
    seller_name TEXT,
    seller_tax_id TEXT,
    buyer_name TEXT,
    buyer_tax_id TEXT,
    currency TEXT NOT NULL DEFAULT 'CNY',
    amount_without_tax TEXT,
    tax_amount TEXT,
    total_amount TEXT,
    category TEXT,
    remarks TEXT,
    extra_fields TEXT,
    source_page_range TEXT,
    confidence REAL,
    has_embedding INTEGER NOT NULL DEFAULT 0,
    status TEXT NOT NULL DEFAULT 'pending_confirmation',
    duplicate_status TEXT NOT NULL DEFAULT 'unknown',
    viewed_at TEXT,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (raw_file_id) REFERENCES raw_files(id)
);

CREATE INDEX IF NOT EXISTS idx_invoices_issue_date ON invoices(issue_date);
CREATE INDEX IF NOT EXISTS idx_invoices_seller_name ON invoices(seller_name);
CREATE INDEX IF NOT EXISTS idx_invoices_buyer_name ON invoices(buyer_name);
CREATE INDEX IF NOT EXISTS idx_invoices_invoice_number ON invoices(invoice_number);
CREATE INDEX IF NOT EXISTS idx_invoices_total_amount ON invoices(total_amount);
CREATE INDEX IF NOT EXISTS idx_invoices_status ON invoices(status);
CREATE INDEX IF NOT EXISTS idx_invoices_duplicate_status ON invoices(duplicate_status);
CREATE INDEX IF NOT EXISTS idx_invoices_viewed_at ON invoices(viewed_at);

-- invoice_items
CREATE TABLE IF NOT EXISTS invoice_items (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    invoice_id INTEGER NOT NULL,
    name TEXT NOT NULL,
    specification TEXT,
    unit TEXT,
    quantity TEXT,
    unit_price TEXT,
    amount TEXT,
    tax_rate TEXT,
    tax_amount TEXT,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (invoice_id) REFERENCES invoices(id) ON DELETE CASCADE
);

-- extraction_runs
CREATE TABLE IF NOT EXISTS extraction_runs (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    raw_file_id INTEGER NOT NULL,
    invoice_id INTEGER,
    provider_name TEXT NOT NULL,
    model TEXT NOT NULL,
    status TEXT NOT NULL,
    request_started_at TEXT NOT NULL,
    duration_ms INTEGER,
    prompt_tokens INTEGER,
    completion_tokens INTEGER,
    response_summary TEXT,
    error_message TEXT,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (raw_file_id) REFERENCES raw_files(id),
    FOREIGN KEY (invoice_id) REFERENCES invoices(id)
);

-- dedupe_candidates
CREATE TABLE IF NOT EXISTS dedupe_candidates (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    invoice_id INTEGER NOT NULL,
    candidate_invoice_id INTEGER NOT NULL,
    score REAL NOT NULL DEFAULT 0.0,
    reason TEXT NOT NULL DEFAULT 'field_match',
    status TEXT NOT NULL DEFAULT 'open',
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    resolved_at TEXT,
    FOREIGN KEY (invoice_id) REFERENCES invoices(id) ON DELETE CASCADE,
    FOREIGN KEY (candidate_invoice_id) REFERENCES invoices(id) ON DELETE CASCADE,
    UNIQUE(invoice_id, candidate_invoice_id)
);

CREATE INDEX IF NOT EXISTS idx_dedupe_candidates_invoice ON dedupe_candidates(invoice_id);
CREATE INDEX IF NOT EXISTS idx_dedupe_candidates_status ON dedupe_candidates(status);

-- watch_dirs
CREATE TABLE IF NOT EXISTS watch_dirs (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    path TEXT NOT NULL UNIQUE,
    extensions TEXT NOT NULL DEFAULT '',
    recursive INTEGER NOT NULL DEFAULT 1,
    enabled INTEGER NOT NULL DEFAULT 1,
    stable_wait_ms INTEGER NOT NULL DEFAULT 2000,
    archive_after_import INTEGER NOT NULL DEFAULT 0,
    archive_path TEXT,
    name_keywords TEXT NOT NULL DEFAULT '',
    max_file_age_days INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

-- agent_sessions
CREATE TABLE IF NOT EXISTS agent_sessions (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    title TEXT NOT NULL DEFAULT '新对话',
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

-- agent_messages
CREATE TABLE IF NOT EXISTS agent_messages (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    session_id INTEGER NOT NULL REFERENCES agent_sessions(id) ON DELETE CASCADE,
    role TEXT NOT NULL CHECK (role IN ('user', 'assistant', 'tool')),
    content TEXT NOT NULL DEFAULT '',
    tool_call_json TEXT,
    tool_call_id TEXT,
    reasoning_content TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

-- audit_logs
CREATE TABLE IF NOT EXISTS audit_logs (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    actor TEXT NOT NULL DEFAULT 'agent',
    action TEXT NOT NULL,
    target_type TEXT,
    target_id INTEGER,
    payload_json TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

-- agent_attachments
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

-- agent_tasks
CREATE TABLE IF NOT EXISTS agent_tasks (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    session_id INTEGER NOT NULL REFERENCES agent_sessions(id) ON DELETE CASCADE,
    tool_name TEXT NOT NULL,
    status TEXT NOT NULL,
    input_json TEXT,
    result_json TEXT,
    error_message TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now')),
    completed_at TEXT
);

CREATE INDEX IF NOT EXISTS idx_agent_tasks_session_id ON agent_tasks(session_id);
CREATE INDEX IF NOT EXISTS idx_agent_tasks_status ON agent_tasks(status);

-- agent_artifacts
CREATE TABLE IF NOT EXISTS agent_artifacts (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    session_id INTEGER NOT NULL REFERENCES agent_sessions(id) ON DELETE CASCADE,
    task_id INTEGER REFERENCES agent_tasks(id) ON DELETE SET NULL,
    artifact_type TEXT NOT NULL,
    title TEXT NOT NULL,
    file_path TEXT,
    mime_type TEXT,
    byte_size INTEGER,
    metadata_json TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_agent_artifacts_session_id ON agent_artifacts(session_id);
CREATE INDEX IF NOT EXISTS idx_agent_artifacts_task_id ON agent_artifacts(task_id);

-- invoice_embeddings
CREATE TABLE IF NOT EXISTS invoice_embeddings (
    invoice_id INTEGER PRIMARY KEY REFERENCES invoices(id) ON DELETE CASCADE,
    embedding BLOB NOT NULL,
    text_content TEXT NOT NULL DEFAULT '',
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

-- events
CREATE TABLE IF NOT EXISTS events (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    event_type TEXT NOT NULL,
    title TEXT NOT NULL,
    description TEXT NOT NULL DEFAULT '',
    status TEXT NOT NULL DEFAULT 'completed' CHECK (status IN ('pending', 'running', 'completed', 'failed')),
    is_read INTEGER NOT NULL DEFAULT 0,
    reference_type TEXT,
    reference_id INTEGER,
    metadata_json TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_events_type ON events(event_type);
CREATE INDEX IF NOT EXISTS idx_events_created_at ON events(created_at);
CREATE INDEX IF NOT EXISTS idx_events_is_read ON events(is_read);

-- notifications
CREATE TABLE IF NOT EXISTS notifications (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    level TEXT NOT NULL CHECK (level IN ('info', 'warning', 'error')),
    title TEXT NOT NULL,
    message TEXT NOT NULL DEFAULT '',
    is_read INTEGER NOT NULL DEFAULT 0,
    reference_type TEXT,
    reference_id INTEGER,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_notifications_read ON notifications(is_read);
CREATE INDEX IF NOT EXISTS idx_notifications_created_at ON notifications(created_at);

-- invoice_badges
CREATE TABLE IF NOT EXISTS invoice_badges (
    invoice_id INTEGER NOT NULL,
    group_name TEXT NOT NULL,
    value TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY (invoice_id, group_name),
    FOREIGN KEY (invoice_id) REFERENCES invoices(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_invoice_badges_invoice_id ON invoice_badges(invoice_id);

-- usage_log
CREATE TABLE IF NOT EXISTS usage_log (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    operation TEXT NOT NULL,
    model TEXT,
    prompt_tokens INTEGER NOT NULL DEFAULT 0,
    completion_tokens INTEGER NOT NULL DEFAULT 0,
    total_tokens INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_usage_log_created_at ON usage_log(created_at);
CREATE INDEX IF NOT EXISTS idx_usage_log_operation ON usage_log(operation);

-- email_sources
CREATE TABLE IF NOT EXISTS email_sources (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL DEFAULT '',
    protocol TEXT NOT NULL DEFAULT 'imap',
    imap_host TEXT NOT NULL,
    imap_port INTEGER NOT NULL DEFAULT 993,
    username TEXT NOT NULL,
    password TEXT NOT NULL,
    use_ssl INTEGER NOT NULL DEFAULT 1,
    folder TEXT NOT NULL DEFAULT 'INBOX',
    name_keywords TEXT NOT NULL DEFAULT '',
    max_email_age_days INTEGER NOT NULL DEFAULT 0,
    enabled INTEGER NOT NULL DEFAULT 1,
    last_uid INTEGER NOT NULL DEFAULT 0,
    poll_interval_seconds INTEGER NOT NULL DEFAULT 60,
    processed_uidls TEXT NOT NULL DEFAULT '',
    last_sync_at TEXT,
    status TEXT NOT NULL DEFAULT 'idle',
    error_message TEXT,
    auth_method TEXT NOT NULL DEFAULT 'password',
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);
