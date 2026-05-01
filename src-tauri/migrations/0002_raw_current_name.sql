ALTER TABLE raw_files ADD COLUMN current_name TEXT;

UPDATE raw_files
SET current_name = original_name
WHERE current_name IS NULL;
