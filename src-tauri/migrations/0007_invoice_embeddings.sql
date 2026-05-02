CREATE TABLE invoice_embeddings (
    invoice_id INTEGER PRIMARY KEY REFERENCES invoices(id) ON DELETE CASCADE,
    embedding BLOB NOT NULL,
    text_content TEXT NOT NULL DEFAULT '',
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);
