ALTER TABLE capture_jobs
    ADD COLUMN lease_owner TEXT,
    ADD COLUMN lease_id TEXT,
    ADD COLUMN lease_epoch BIGINT NOT NULL DEFAULT 0 CHECK (lease_epoch >= 0),
    ADD COLUMN lease_expires_at TIMESTAMPTZ,
    ADD CONSTRAINT capture_jobs_lease_shape CHECK (
        (
            lease_owner IS NULL
            AND lease_id IS NULL
            AND lease_expires_at IS NULL
            AND lease_epoch = 0
        )
        OR (
            lease_owner IS NOT NULL
            AND lease_id IS NOT NULL
            AND lease_expires_at IS NOT NULL
            AND length(lease_owner) BETWEEN 1 AND 128
            AND length(lease_id) BETWEEN 1 AND 128
            AND lease_epoch > 0
        )
    );

CREATE INDEX capture_jobs_claimable
    ON capture_jobs (state, lease_expires_at, created_at)
    WHERE state NOT IN ('completed', 'failed', 'canceled');
