CREATE TABLE IF NOT EXISTS session_share_sync_state (
  viewer_user_id                 TEXT NOT NULL CHECK (
    viewer_user_id = trim(viewer_user_id)
    AND length(viewer_user_id) > 0
    AND length(viewer_user_id) <= 512
  ),
  share_id                       TEXT NOT NULL CHECK (
    share_id = trim(share_id)
    AND length(share_id) > 0
    AND length(share_id) <= 512
  ),
  session_id                     TEXT NOT NULL CHECK (
    session_id = trim(session_id)
    AND length(session_id) > 0
    AND length(session_id) <= 512
  ),
  acknowledged_content_revision INTEGER NOT NULL CHECK (
    acknowledged_content_revision > 0
  ),
  baseline_source_hash           TEXT NOT NULL CHECK (
    length(baseline_source_hash) = 64
    AND baseline_source_hash = lower(baseline_source_hash)
    AND baseline_source_hash NOT GLOB '*[^0-9a-f]*'
  ),
  status                         TEXT NOT NULL DEFAULT 'clean' CHECK (
    status IN ('clean', 'conflict')
  ),
  updated_at                     TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  PRIMARY KEY (viewer_user_id, share_id)
) STRICT;

CREATE INDEX IF NOT EXISTS idx_session_share_sync_state_session
ON session_share_sync_state(viewer_user_id, session_id);
