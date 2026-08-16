CREATE TABLE capture_jobs (
    workspace_id TEXT NOT NULL CHECK (length(workspace_id) BETWEEN 1 AND 128),
    job_id TEXT NOT NULL CHECK (length(job_id) BETWEEN 1 AND 128),
    bot_id TEXT NOT NULL CHECK (length(bot_id) BETWEEN 1 AND 128),
    owner_user_id TEXT NOT NULL CHECK (length(owner_user_id) BETWEEN 1 AND 128),
    requesting_actor_id TEXT NOT NULL CHECK (length(requesting_actor_id) BETWEEN 1 AND 128),
    session_id TEXT NOT NULL CHECK (length(session_id) BETWEEN 1 AND 128),
    session_title TEXT NOT NULL CHECK (length(session_title) BETWEEN 1 AND 1024),
    provider TEXT NOT NULL,
    meeting JSONB NOT NULL,
    state TEXT NOT NULL DEFAULT 'queued',
    terminal_reason JSONB,
    last_sequence BIGINT NOT NULL DEFAULT -1 CHECK (last_sequence >= -1),
    created_at TIMESTAMPTZ NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL,
    PRIMARY KEY (workspace_id, job_id),
    UNIQUE (workspace_id, bot_id)
);

CREATE TABLE capture_events (
    workspace_id TEXT NOT NULL,
    job_id TEXT NOT NULL,
    sequence BIGINT NOT NULL CHECK (sequence >= 0),
    event_id TEXT NOT NULL CHECK (length(event_id) BETWEEN 1 AND 128),
    occurred_at TIMESTAMPTZ NOT NULL,
    event JSONB NOT NULL,
    persisted_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (workspace_id, job_id, sequence),
    UNIQUE (workspace_id, job_id, event_id),
    FOREIGN KEY (workspace_id, job_id)
        REFERENCES capture_jobs (workspace_id, job_id)
        ON DELETE CASCADE
);

CREATE TABLE capture_projection_checkpoints (
    workspace_id TEXT NOT NULL,
    job_id TEXT NOT NULL,
    last_sequence BIGINT NOT NULL CHECK (last_sequence >= 0),
    revision BIGINT NOT NULL CHECK (revision > 0),
    content_hash TEXT NOT NULL CHECK (content_hash ~ '^[0-9a-f]{64}$'),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (workspace_id, job_id),
    FOREIGN KEY (workspace_id, job_id)
        REFERENCES capture_jobs (workspace_id, job_id)
        ON DELETE CASCADE,
    FOREIGN KEY (workspace_id, job_id, revision)
        REFERENCES session_envelopes (workspace_id, job_id, revision)
        ON DELETE CASCADE
);
