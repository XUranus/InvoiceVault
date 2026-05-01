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
