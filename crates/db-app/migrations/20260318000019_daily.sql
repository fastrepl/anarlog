CREATE TABLE IF NOT EXISTS daily (
  id         TEXT PRIMARY KEY NOT NULL,
  date       TEXT NOT NULL DEFAULT '',
  body       TEXT NOT NULL DEFAULT '{}',
  user_id    TEXT NOT NULL DEFAULT '',
  visibility TEXT NOT NULL DEFAULT 'public',
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
  updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
);
CREATE UNIQUE INDEX IF NOT EXISTS idx_daily_date_user ON daily(date, user_id);
