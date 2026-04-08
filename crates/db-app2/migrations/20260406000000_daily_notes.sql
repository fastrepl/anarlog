CREATE TABLE IF NOT EXISTS daily_notes (
  id         TEXT PRIMARY KEY NOT NULL,
  date       TEXT NOT NULL DEFAULT '',
  content    TEXT NOT NULL DEFAULT '{}',
  user_id    TEXT NOT NULL DEFAULT '',
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
  updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_daily_notes_date_user ON daily_notes(date, user_id);
