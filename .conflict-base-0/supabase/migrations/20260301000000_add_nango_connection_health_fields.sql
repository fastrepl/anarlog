ALTER TABLE "nango_connections"
    ADD COLUMN "status" text NOT NULL DEFAULT 'connected',
    ADD COLUMN "last_error_type" text,
    ADD COLUMN "last_error_description" text,
    ADD COLUMN "last_error_at" timestamptz;

ALTER TABLE "nango_connections"
    ADD CONSTRAINT "nango_connections_status_check"
    CHECK ("status" IN ('connected', 'reconnect_required'));

CREATE INDEX "nango_connections_status_idx"
    ON "nango_connections" ("status");
