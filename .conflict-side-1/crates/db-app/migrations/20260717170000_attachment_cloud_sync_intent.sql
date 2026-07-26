ALTER TABLE session_attachments
ADD COLUMN cloud_sync_enabled INTEGER NOT NULL DEFAULT 0
CHECK (cloud_sync_enabled IN (0, 1));
