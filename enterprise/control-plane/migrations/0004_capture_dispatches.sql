ALTER TABLE capture_jobs
    ADD COLUMN dispatch_id TEXT,
    ADD COLUMN dispatch_payload JSONB,
    ADD COLUMN dispatch_accepted_at TIMESTAMPTZ,
    ADD CONSTRAINT capture_jobs_dispatch_shape CHECK (
        (
            dispatch_id IS NULL
            AND dispatch_payload IS NULL
            AND dispatch_accepted_at IS NULL
        )
        OR (
            dispatch_id IS NOT NULL
            AND length(dispatch_id) BETWEEN 1 AND 512
            AND dispatch_payload IS NOT NULL
            AND dispatch_accepted_at IS NOT NULL
        )
    );

CREATE UNIQUE INDEX capture_jobs_provider_dispatch
    ON capture_jobs (provider, dispatch_id)
    WHERE dispatch_id IS NOT NULL;
