ALTER TABLE events ADD COLUMN is_read INTEGER NOT NULL DEFAULT 0;
CREATE INDEX IF NOT EXISTS idx_events_is_read ON events(is_read);
