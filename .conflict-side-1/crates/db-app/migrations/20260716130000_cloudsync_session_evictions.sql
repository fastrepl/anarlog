CREATE TABLE IF NOT EXISTS cloudsync_session_evictions (
  session_id      TEXT PRIMARY KEY NOT NULL,
  workspace_id    TEXT NOT NULL,
  queued_at       TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  attempt_count   INTEGER NOT NULL DEFAULT 0,
  last_attempt_at TEXT,
  last_error      TEXT NOT NULL DEFAULT ''
) STRICT;

CREATE INDEX IF NOT EXISTS idx_cloudsync_session_evictions_workspace_id
ON cloudsync_session_evictions(workspace_id);

CREATE TABLE IF NOT EXISTS cloudsync_writable_workspaces (
  allowed_workspace_id TEXT PRIMARY KEY NOT NULL
) STRICT;
