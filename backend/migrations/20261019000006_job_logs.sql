CREATE TABLE job_logs (
    id bigserial PRIMARY KEY,
    job_id uuid NOT NULL,
    level text NOT NULL,
    message text NOT NULL,
    created_at timestamptz NOT NULL DEFAULT now()
);
CREATE INDEX job_logs_job_time ON job_logs(job_id,id);
CREATE INDEX job_logs_created ON job_logs(created_at);
