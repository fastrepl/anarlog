CREATE TABLE IF NOT EXISTS shared_session_cache (
  share_id          TEXT NOT NULL CHECK (
    share_id = trim(share_id) AND length(share_id) > 0
  ),
  viewer_user_id    TEXT NOT NULL CHECK (
    viewer_user_id = trim(viewer_user_id) AND length(viewer_user_id) > 0
  ),
  workspace_id      TEXT NOT NULL CHECK (
    workspace_id = trim(workspace_id) AND length(workspace_id) > 0
  ),
  session_id        TEXT NOT NULL CHECK (
    session_id = trim(session_id) AND length(session_id) > 0
  ),
  schema_version    INTEGER NOT NULL DEFAULT 1 CHECK (schema_version = 1),
  content_revision  INTEGER NOT NULL CHECK (content_revision > 0),
  title             TEXT NOT NULL DEFAULT '' CHECK (
    title = trim(title) AND length(CAST(title AS BLOB)) <= 4096
  ),
  body_json         TEXT NOT NULL DEFAULT '{"type":"doc","content":[{"type":"paragraph"}]}' CHECK (
    CASE
      WHEN json_valid(body_json) THEN
        json_type(body_json) = 'object'
        AND json_extract(body_json, '$.type') = 'doc'
        AND length(CAST(body_json AS BLOB)) <= 2097152
      ELSE 0
    END
  ),
  capability        TEXT NOT NULL DEFAULT 'viewer' CHECK (
    capability IN ('viewer', 'commenter', 'editor')
  ),
  manage_access     INTEGER NOT NULL DEFAULT 0 CHECK (manage_access IN (0, 1)),
  access_version    INTEGER NOT NULL CHECK (access_version > 0),
  published_at      TEXT NOT NULL CHECK (
    published_at = trim(published_at) AND length(published_at) > 0
  ),
  cached_at         TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  PRIMARY KEY (viewer_user_id, share_id)
) STRICT;

CREATE INDEX IF NOT EXISTS idx_shared_session_cache_viewer_workspace
ON shared_session_cache(viewer_user_id, workspace_id);
