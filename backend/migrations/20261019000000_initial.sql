CREATE TABLE streamers (
    id uuid PRIMARY KEY,
    login varchar(25) NOT NULL UNIQUE,
    enabled boolean NOT NULL DEFAULT true,
    quality varchar(20) NOT NULL DEFAULT '720p,best',
    priority integer NOT NULL DEFAULT 0 CHECK (priority BETWEEN 0 AND 100),
    pre_roll integer NOT NULL DEFAULT 30,
    post_roll integer NOT NULL DEFAULT 20,
    threshold_db double precision NOT NULL DEFAULT 9,
    auto_clip boolean NOT NULL DEFAULT true,
    status varchar(20) NOT NULL DEFAULT 'waiting',
    last_error text,
    next_check_at timestamptz,
    created_at timestamptz NOT NULL DEFAULT now(),
    deleted_at timestamptz
);
CREATE TABLE recordings (
    id uuid PRIMARY KEY,
    streamer_id uuid NOT NULL REFERENCES streamers(id),
    started_at timestamptz NOT NULL DEFAULT now(),
    ended_at timestamptz,
    latest_ms bigint NOT NULL DEFAULT 0,
    audio_db double precision,
    error text
);
CREATE INDEX recordings_streamer ON recordings(streamer_id);
CREATE UNIQUE INDEX recordings_one_active ON recordings(streamer_id) WHERE ended_at IS NULL;
CREATE TABLE segments (
    id bigserial PRIMARY KEY,
    recording_id uuid NOT NULL REFERENCES recordings(id),
    path text NOT NULL UNIQUE,
    start_ms bigint NOT NULL,
    end_ms bigint NOT NULL,
    size_bytes bigint NOT NULL
);
CREATE INDEX segments_recording_time ON segments(recording_id, end_ms);
CREATE TABLE clips (
    id uuid PRIMARY KEY,
    streamer_id uuid NOT NULL REFERENCES streamers(id),
    recording_id uuid NOT NULL REFERENCES recordings(id),
    title varchar(200) NOT NULL,
    status varchar(20) NOT NULL DEFAULT 'collecting',
    trigger varchar(20) NOT NULL DEFAULT 'audio',
    score double precision NOT NULL DEFAULT 0,
    reason text NOT NULL DEFAULT '',
    start_ms bigint NOT NULL,
    end_ms bigint NOT NULL,
    manifest jsonb NOT NULL DEFAULT '[]',
    video_key text,
    thumbnail_key text,
    size_bytes bigint NOT NULL DEFAULT 0,
    favorite boolean NOT NULL DEFAULT false,
    rating integer NOT NULL DEFAULT 0 CHECK (rating BETWEEN -1 AND 1),
    attempts integer NOT NULL DEFAULT 0,
    error text,
    retry_at timestamptz,
    created_at timestamptz NOT NULL DEFAULT now(),
    ready_at timestamptz
);
CREATE INDEX clips_streamer ON clips(streamer_id);
CREATE INDEX clips_recording ON clips(recording_id);
CREATE INDEX clips_status_created ON clips(status, created_at);
CREATE TABLE activities (
    id bigserial PRIMARY KEY,
    streamer_id uuid REFERENCES streamers(id),
    kind varchar(30) NOT NULL,
    message text NOT NULL,
    created_at timestamptz NOT NULL DEFAULT now()
);
CREATE INDEX activities_created ON activities(created_at);
CREATE TABLE app_settings (id integer PRIMARY KEY CHECK (id = 1), values jsonb NOT NULL DEFAULT '{}');
CREATE TABLE worker_state (id integer PRIMARY KEY CHECK (id = 1), heartbeat_at timestamptz NOT NULL DEFAULT now(), details jsonb NOT NULL DEFAULT '{}');
