ALTER TABLE streamers ADD COLUMN trigger_profile jsonb NOT NULL DEFAULT '{"version":1,"warmup_seconds":60,"cooldown_seconds":120,"hourly_limit":6,"rules":[{"name":"Reaktion mit Chat","enabled":true,"hold_seconds":1.5,"conditions":[{"metric":"audio_rise","op":"gte","value":8},{"metric":"audio_level","op":"gte","value":-24},{"metric":"chat_messages","op":"gte","value":5},{"metric":"chat_users","op":"gte","value":3}]}]}';
ALTER TABLE recordings ADD COLUMN trigger_state jsonb NOT NULL DEFAULT '{}';
CREATE TABLE trigger_samples (
    recording_id uuid NOT NULL REFERENCES recordings(id) ON DELETE CASCADE,
    start_ms bigint NOT NULL,
    frames jsonb NOT NULL,
    created_at timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY (recording_id, start_ms)
);
CREATE INDEX trigger_samples_created ON trigger_samples(created_at);
-- Separate from clips: deleting a clip or restarting must not reset the rate limit.
CREATE TABLE trigger_events (
    id bigserial PRIMARY KEY,
    streamer_id uuid NOT NULL REFERENCES streamers(id),
    created_at timestamptz NOT NULL DEFAULT now()
);
CREATE INDEX trigger_events_streamer_time ON trigger_events(streamer_id, created_at);
