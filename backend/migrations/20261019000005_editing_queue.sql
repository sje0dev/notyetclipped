CREATE TABLE editing_profiles (
    streamer_id uuid PRIMARY KEY REFERENCES streamers(id),
    profile jsonb NOT NULL DEFAULT '{}'
);
CREATE TABLE edits (
    id uuid PRIMARY KEY,
    clip_id uuid NOT NULL REFERENCES clips(id) ON DELETE CASCADE,
    profile jsonb NOT NULL,
    status text NOT NULL DEFAULT 'queued',
    video_key text NOT NULL,
    size_bytes bigint NOT NULL DEFAULT 0,
    error text,
    created_at timestamptz NOT NULL DEFAULT now(),
    finished_at timestamptz
);
CREATE UNIQUE INDEX edits_one_active ON edits(clip_id) WHERE status IN ('queued','rendering');
CREATE INDEX edits_created ON edits(created_at DESC);
