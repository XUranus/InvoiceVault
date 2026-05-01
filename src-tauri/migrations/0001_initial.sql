CREATE TABLE IF NOT EXISTS raw_files (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    sha256 TEXT NOT NULL UNIQUE,
    md5 TEXT,
    original_name TEXT NOT NULL,
    extension TEXT NOT NULL,
    mime_type TEXT,
    byte_size INTEGER NOT NULL,
    storage_path TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE IF NOT EXISTS import_jobs (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    raw_file_id INTEGER,
    source_path TEXT NOT NULL,
    status TEXT NOT NULL,
    error_message TEXT,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (raw_file_id) REFERENCES raw_files(id)
);

CREATE INDEX IF NOT EXISTS idx_import_jobs_status ON import_jobs(status);
CREATE INDEX IF NOT EXISTS idx_import_jobs_created_at ON import_jobs(created_at);

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
    source_page_range TEXT,
    confidence REAL,
    status TEXT NOT NULL DEFAULT 'pending_confirmation',
    duplicate_status TEXT NOT NULL DEFAULT 'unknown',
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

