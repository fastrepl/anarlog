CREATE TABLE IF NOT EXISTS calendars (
  id TEXT PRIMARY KEY NOT NULL,
  provider TEXT NOT NULL DEFAULT '',
  connection_id TEXT NOT NULL DEFAULT '',
  tracking_id TEXT NOT NULL DEFAULT '',
  name TEXT NOT NULL DEFAULT '',
  color TEXT NOT NULL DEFAULT '',
  source TEXT NOT NULL DEFAULT '',
  enabled INTEGER NOT NULL DEFAULT 0,
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
  user_id TEXT NOT NULL DEFAULT '',
  raw_json TEXT NOT NULL DEFAULT '{}'
);
CREATE INDEX IF NOT EXISTS idx_calendars_connection ON calendars(connection_id);
