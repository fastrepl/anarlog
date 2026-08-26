-- Local proposal inbox for CLI, MCP, and in-app chat.
-- Older builds ignore this table. Not CloudSync-enabled.
CREATE TABLE IF NOT EXISTS session_proposals (
  id                  TEXT PRIMARY KEY NOT NULL,
  workspace_id        TEXT NOT NULL DEFAULT '',
  session_id          TEXT NOT NULL DEFAULT '',
  kind                TEXT NOT NULL DEFAULT '',
  target_id           TEXT NOT NULL DEFAULT '',
  base_updated_at     TEXT NOT NULL DEFAULT '',
  current_markdown    TEXT NOT NULL DEFAULT '',
  proposed_markdown   TEXT NOT NULL DEFAULT '',
  status              TEXT NOT NULL DEFAULT 'pending',
  source              TEXT NOT NULL DEFAULT '',
  created_at          TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  updated_at          TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
) STRICT;

CREATE INDEX IF NOT EXISTS idx_session_proposals_session_status
  ON session_proposals (session_id, status, created_at);

CREATE INDEX IF NOT EXISTS idx_session_proposals_status_created
  ON session_proposals (status, created_at);
