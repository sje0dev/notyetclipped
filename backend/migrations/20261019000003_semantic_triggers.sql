CREATE TABLE ai_control (
    id integer PRIMARY KEY CHECK (id = 1),
    settings jsonb NOT NULL DEFAULT '{"enabled":false,"model":"","hourly_limit":20,"daily_limit":100}',
    paused_reason text
);
INSERT INTO ai_control(id) VALUES (1);

CREATE TABLE ai_runs (
    id uuid PRIMARY KEY,
    recording_id uuid NOT NULL REFERENCES recordings(id) ON DELETE CASCADE,
    streamer_id uuid NOT NULL REFERENCES streamers(id),
    start_ms bigint NOT NULL,
    end_ms bigint NOT NULL,
    status text NOT NULL DEFAULT 'preparing',
    message text NOT NULL DEFAULT '',
    result jsonb,
    clip_id uuid REFERENCES clips(id) ON DELETE SET NULL,
    created_at timestamptz NOT NULL DEFAULT now(),
    sent_at timestamptz,
    finished_at timestamptz
);
CREATE INDEX ai_runs_recording_idx ON ai_runs(recording_id, created_at DESC);
CREATE INDEX ai_runs_sent_idx ON ai_runs(sent_at) WHERE sent_at IS NOT NULL;
