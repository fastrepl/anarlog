CREATE TABLE capture_policies (
    workspace_id TEXT PRIMARY KEY CHECK (length(workspace_id) BETWEEN 1 AND 128),
    capture_enabled BOOLEAN NOT NULL DEFAULT false,
    allowed_providers JSONB NOT NULL DEFAULT '["anarlog"]'::jsonb,
    bot_name TEXT NOT NULL DEFAULT 'Anarlog Notetaker'
        CHECK (length(bot_name) BETWEEN 1 AND 80),
    disclosure_text TEXT CHECK (
        disclosure_text IS NULL OR length(disclosure_text) BETWEEN 1 AND 2048
    ),
    skip_if_desktop_capture BOOLEAN NOT NULL DEFAULT true,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp()
);

CREATE TABLE scheduled_captures (
    workspace_id TEXT NOT NULL CHECK (length(workspace_id) BETWEEN 1 AND 128),
    calendar_event_id TEXT NOT NULL CHECK (length(calendar_event_id) BETWEEN 1 AND 512),
    job_id TEXT CHECK (job_id IS NULL OR length(job_id) BETWEEN 1 AND 128),
    title TEXT NOT NULL CHECK (length(title) BETWEEN 1 AND 1024),
    starts_at TIMESTAMPTZ NOT NULL,
    ends_at TIMESTAMPTZ,
    meeting JSONB NOT NULL,
    provider TEXT NOT NULL,
    owner_user_id TEXT NOT NULL CHECK (length(owner_user_id) BETWEEN 1 AND 128),
    status TEXT NOT NULL CHECK (
        status IN ('pending', 'skipped', 'canceled', 'dispatched')
    ),
    skip_reason TEXT CHECK (
        skip_reason IS NULL OR length(skip_reason) BETWEEN 1 AND 128
    ),
    bot_name TEXT NOT NULL CHECK (length(bot_name) BETWEEN 1 AND 80),
    disclosure_text TEXT,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp(),
    PRIMARY KEY (workspace_id, calendar_event_id)
);

CREATE UNIQUE INDEX scheduled_captures_workspace_job
    ON scheduled_captures (workspace_id, job_id)
    WHERE job_id IS NOT NULL;

CREATE INDEX scheduled_captures_dispatchable
    ON scheduled_captures (starts_at)
    WHERE status = 'pending';
