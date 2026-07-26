CREATE TABLE IF NOT EXISTS attachment_local_state (
  attachment_id  TEXT PRIMARY KEY NOT NULL,
  session_id     TEXT NOT NULL DEFAULT '',
  relative_path  TEXT NOT NULL DEFAULT '',
  availability   TEXT NOT NULL DEFAULT 'present'
                 CHECK (availability IN ('present', 'absent')),
  updated_at     TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
) STRICT;

CREATE INDEX IF NOT EXISTS idx_attachment_local_state_session_id
ON attachment_local_state(session_id);
