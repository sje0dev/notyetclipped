ALTER TABLE ai_runs ADD COLUMN preparation_ms bigint;
ALTER TABLE ai_runs ADD COLUMN inference_ms bigint;
CREATE INDEX ai_runs_streamer_created_idx ON ai_runs(streamer_id, created_at DESC);
CREATE INDEX ai_runs_pending_idx ON ai_runs(recording_id) WHERE status IN ('preparing','analyzing');
