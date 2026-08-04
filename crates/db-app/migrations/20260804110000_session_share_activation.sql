CREATE TABLE IF NOT EXISTS session_share_activation (
  viewer_user_id TEXT NOT NULL CHECK (
    viewer_user_id = trim(viewer_user_id) AND length(viewer_user_id) > 0
  ),
  share_id       TEXT NOT NULL CHECK (
    share_id = trim(share_id) AND length(share_id) > 0
  ),
  session_id     TEXT NOT NULL CHECK (
    session_id = trim(session_id) AND length(session_id) > 0
  ),
  activated_at   TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  PRIMARY KEY (viewer_user_id, share_id),
  UNIQUE (viewer_user_id, session_id)
) STRICT;
