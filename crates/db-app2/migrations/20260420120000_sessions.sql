CREATE TABLE IF NOT EXISTS sessions (
  id          TEXT PRIMARY KEY NOT NULL,
  title       TEXT NOT NULL DEFAULT '',
  raw_md      TEXT NOT NULL DEFAULT '',
  folder_id   TEXT NOT NULL DEFAULT '',
  event_json  TEXT NOT NULL DEFAULT '',
  created_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
  updated_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
);

CREATE INDEX IF NOT EXISTS idx_sessions_updated_at ON sessions(updated_at);
