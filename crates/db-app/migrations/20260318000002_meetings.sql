CREATE TABLE IF NOT EXISTS meetings (
  id TEXT PRIMARY KEY NOT NULL,
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
  title TEXT,
  summary TEXT,
  memo TEXT,
  user_id TEXT NOT NULL DEFAULT '',
  visibility TEXT NOT NULL DEFAULT 'public',
  folder_id TEXT DEFAULT NULL,
  event_id TEXT DEFAULT NULL REFERENCES events(id)
);
CREATE INDEX IF NOT EXISTS idx_meetings_event ON meetings(event_id);
