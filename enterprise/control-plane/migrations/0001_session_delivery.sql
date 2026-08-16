CREATE TABLE session_envelopes (
    cursor BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    workspace_id TEXT NOT NULL CHECK (length(workspace_id) BETWEEN 1 AND 128),
    job_id TEXT NOT NULL CHECK (length(job_id) BETWEEN 1 AND 128),
    revision BIGINT NOT NULL CHECK (revision > 0),
    finalized BOOLEAN NOT NULL,
    content_hash TEXT NOT NULL CHECK (content_hash ~ '^[0-9a-f]{64}$'),
    envelope JSONB NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (workspace_id, job_id, revision)
);

CREATE INDEX session_envelopes_workspace_cursor_idx
    ON session_envelopes (workspace_id, cursor);

CREATE INDEX session_envelopes_workspace_job_revision_idx
    ON session_envelopes (workspace_id, job_id, revision DESC);

CREATE TABLE session_envelope_acknowledgements (
    workspace_id TEXT NOT NULL,
    consumer_id TEXT NOT NULL CHECK (length(consumer_id) BETWEEN 1 AND 128),
    job_id TEXT NOT NULL,
    revision BIGINT NOT NULL,
    content_hash TEXT NOT NULL,
    acknowledged_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (workspace_id, consumer_id, job_id, revision),
    FOREIGN KEY (workspace_id, job_id, revision)
        REFERENCES session_envelopes (workspace_id, job_id, revision)
        ON DELETE CASCADE
);
