ALTER TABLE invoices ADD COLUMN viewed_at TEXT;

UPDATE invoices
SET viewed_at = CURRENT_TIMESTAMP
WHERE viewed_at IS NULL;

CREATE INDEX IF NOT EXISTS idx_invoices_viewed_at ON invoices(viewed_at);
