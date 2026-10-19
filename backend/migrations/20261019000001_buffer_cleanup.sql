ALTER TABLE recordings ADD COLUMN buffer_cleaned_at timestamptz;
CREATE INDEX recordings_pending_cleanup ON recordings(id) WHERE buffer_cleaned_at IS NULL;
