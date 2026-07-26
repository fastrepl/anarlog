CREATE TABLE IF NOT EXISTS attachment_transfer_jobs (
  id                  TEXT PRIMARY KEY NOT NULL
                      CHECK (id <> '' AND length(id) <= 512),
  attachment_id       TEXT NOT NULL
                      CHECK (attachment_id <> '' AND length(attachment_id) <= 512),
  session_id          TEXT NOT NULL
                      CHECK (session_id <> '' AND length(session_id) <= 512),
  workspace_id        TEXT NOT NULL
                      CHECK (workspace_id <> '' AND length(workspace_id) <= 512),
  direction           TEXT NOT NULL
                      CHECK (direction IN ('upload', 'download', 'delete')),
  expected_sha256     TEXT NOT NULL
                      CHECK (
                        length(expected_sha256) = 64
                        AND expected_sha256 NOT GLOB '*[^0-9a-f]*'
                      ),
  expected_size_bytes INTEGER NOT NULL DEFAULT 0
                      CHECK (
                        expected_size_bytes >= 0
                        AND expected_size_bytes <= 536870912
                      ),
  ciphertext_sha256   TEXT NOT NULL DEFAULT ''
                      CHECK (
                        ciphertext_sha256 = ''
                        OR (
                          length(ciphertext_sha256) = 64
                          AND ciphertext_sha256 NOT GLOB '*[^0-9a-f]*'
                        )
                      ),
  ciphertext_size_bytes INTEGER NOT NULL DEFAULT 0
                        CHECK (
                          ciphertext_size_bytes >= 0
                          AND ciphertext_size_bytes <= 545259520
                        ),
  remote_object_id    TEXT NOT NULL DEFAULT ''
                      CHECK (length(remote_object_id) <= 1024),
  object_key          TEXT NOT NULL DEFAULT ''
                      CHECK (length(object_key) <= 2048),
  cache_id            TEXT NOT NULL DEFAULT ''
                      CHECK (length(cache_id) <= 1024),
  phase               TEXT NOT NULL DEFAULT 'queued'
                      CHECK (phase IN (
                        'queued',
                        'preparing',
                        'ready',
                        'transferring',
                        'finalizing',
                        'retry_wait',
                        'failed',
                        'completed'
                      )),
  attempt_count       INTEGER NOT NULL DEFAULT 0
                      CHECK (attempt_count >= 0),
  next_attempt_at     TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  last_attempt_at     TEXT,
  last_error          TEXT NOT NULL DEFAULT ''
                      CHECK (length(last_error) <= 2048),
  created_at          TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  updated_at          TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  completed_at        TEXT,
  CHECK (direction = 'upload' OR object_key <> ''),
  CHECK (
    (ciphertext_sha256 = '' AND ciphertext_size_bytes = 0)
    OR (ciphertext_sha256 <> '' AND ciphertext_size_bytes > 0)
  )
) STRICT;

CREATE UNIQUE INDEX IF NOT EXISTS idx_attachment_transfer_jobs_live_version
ON attachment_transfer_jobs (
  attachment_id,
  expected_sha256,
  expected_size_bytes
)
WHERE direction IN ('upload', 'download')
  AND phase <> 'completed';

CREATE UNIQUE INDEX IF NOT EXISTS idx_attachment_transfer_jobs_delete_object
ON attachment_transfer_jobs(object_key)
WHERE direction = 'delete' AND phase <> 'completed';

CREATE UNIQUE INDEX IF NOT EXISTS idx_attachment_transfer_jobs_upload_object_id
ON attachment_transfer_jobs(remote_object_id)
WHERE direction = 'upload'
  AND remote_object_id <> ''
  AND phase <> 'completed';

CREATE UNIQUE INDEX IF NOT EXISTS idx_attachment_transfer_jobs_delete_object_id
ON attachment_transfer_jobs(remote_object_id)
WHERE direction = 'delete'
  AND remote_object_id <> ''
  AND phase <> 'completed';

CREATE UNIQUE INDEX IF NOT EXISTS idx_attachment_transfer_jobs_cache_id
ON attachment_transfer_jobs(cache_id)
WHERE cache_id <> '';

CREATE INDEX IF NOT EXISTS idx_attachment_transfer_jobs_due
ON attachment_transfer_jobs(phase, next_attempt_at, created_at);

CREATE INDEX IF NOT EXISTS idx_attachment_transfer_jobs_attachment_id
ON attachment_transfer_jobs(attachment_id);

CREATE INDEX IF NOT EXISTS idx_attachment_transfer_jobs_session_id
ON attachment_transfer_jobs(session_id);
