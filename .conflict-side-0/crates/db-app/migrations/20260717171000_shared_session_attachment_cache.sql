CREATE TABLE IF NOT EXISTS shared_session_attachment_cache (
  viewer_user_id    TEXT NOT NULL
                    CHECK (viewer_user_id <> '' AND length(viewer_user_id) <= 512),
  share_id          TEXT NOT NULL
                    CHECK (share_id <> '' AND length(share_id) <= 512),
  attachment_id     TEXT NOT NULL
                    CHECK (attachment_id <> '' AND length(attachment_id) <= 512),
  filename          TEXT NOT NULL DEFAULT ''
                    CHECK (length(filename) <= 1024),
  content_type      TEXT NOT NULL DEFAULT 'application/octet-stream'
                    CHECK (content_type <> '' AND length(content_type) <= 512),
  size_bytes        INTEGER NOT NULL DEFAULT 0
                    CHECK (size_bytes >= 0 AND size_bytes <= 536870912),
  sha256            TEXT NOT NULL
                    CHECK (
                      length(sha256) = 64
                      AND sha256 NOT GLOB '*[^0-9a-f]*'
                    ),
  cache_id          TEXT NOT NULL DEFAULT ''
                    CHECK (length(cache_id) <= 1024),
  claim_token       TEXT NOT NULL DEFAULT ''
                    CHECK (length(claim_token) <= 128),
  cache_generation  INTEGER NOT NULL DEFAULT 0
                    CHECK (cache_generation >= 0),
  availability      TEXT NOT NULL DEFAULT 'pending'
                    CHECK (availability IN (
                      'pending',
                      'downloading',
                      'present',
                      'delete_pending',
                      'deleting',
                      'failed'
                    )),
  access_version    INTEGER NOT NULL DEFAULT 0
                    CHECK (access_version >= 0),
  attempt_count     INTEGER NOT NULL DEFAULT 0
                    CHECK (attempt_count >= 0),
  next_attempt_at   TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  last_attempt_at   TEXT,
  last_error        TEXT NOT NULL DEFAULT ''
                    CHECK (length(last_error) <= 2048),
  created_at        TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  updated_at        TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  PRIMARY KEY (viewer_user_id, share_id, attachment_id)
) STRICT;

CREATE UNIQUE INDEX IF NOT EXISTS idx_shared_session_attachment_cache_cache_id
ON shared_session_attachment_cache(cache_id)
WHERE cache_id <> '';

CREATE INDEX IF NOT EXISTS idx_shared_session_attachment_cache_cleanup
ON shared_session_attachment_cache(availability, updated_at)
WHERE availability IN ('delete_pending', 'deleting', 'failed');

CREATE INDEX IF NOT EXISTS idx_shared_session_attachment_cache_due
ON shared_session_attachment_cache(availability, next_attempt_at, updated_at)
WHERE availability IN ('pending', 'delete_pending', 'failed');

CREATE INDEX IF NOT EXISTS idx_shared_session_attachment_cache_share
ON shared_session_attachment_cache(viewer_user_id, share_id);
