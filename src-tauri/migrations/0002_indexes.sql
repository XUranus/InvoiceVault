-- Performance: add missing indexes on foreign key and hot-path columns

-- invoices.raw_file_id: used in raw_file_has_invoices, batch_delete_invoices,
-- invoice_id_for_raw_file, and correlated subqueries in search
CREATE INDEX IF NOT EXISTS idx_invoices_raw_file_id ON invoices(raw_file_id);

-- invoice_items.invoice_id: used in GROUP_CONCAT subquery in search and in
-- get_invoice_detail. SQLite does not auto-index FK columns.
CREATE INDEX IF NOT EXISTS idx_invoice_items_invoice_id ON invoice_items(invoice_id);

-- extraction_runs: queried in get_invoice_detail
CREATE INDEX IF NOT EXISTS idx_extraction_runs_invoice_id ON extraction_runs(invoice_id);
CREATE INDEX IF NOT EXISTS idx_extraction_runs_raw_file_id ON extraction_runs(raw_file_id);

-- import_jobs.raw_file_id: queried in get_invoice_detail
CREATE INDEX IF NOT EXISTS idx_import_jobs_raw_file_id ON import_jobs(raw_file_id);

-- events.reference: used in batch_delete_invoices cleanup
CREATE INDEX IF NOT EXISTS idx_events_reference ON events(reference_type, reference_id);

-- dedupe_candidates.candidate_invoice_id: used in batch_delete_invoices
CREATE INDEX IF NOT EXISTS idx_dedupe_candidates_candidate ON dedupe_candidates(candidate_invoice_id);
