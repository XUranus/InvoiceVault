-- 监听目录扩展字段
ALTER TABLE watch_dirs ADD COLUMN name_keywords TEXT NOT NULL DEFAULT '';
ALTER TABLE watch_dirs ADD COLUMN max_file_age_days INTEGER NOT NULL DEFAULT 0;

-- 邮件数据源表
CREATE TABLE IF NOT EXISTS email_sources (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL DEFAULT '',
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
    last_sync_at TEXT,
    status TEXT NOT NULL DEFAULT 'idle',
    error_message TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);
