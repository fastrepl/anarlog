CREATE TABLE IF NOT EXISTS sessions (
  id TEXT PRIMARY KEY,
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
  visited_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
  user_id TEXT NOT NULL,
  calendar_event_id TEXT DEFAULT NULL UNIQUE,
  title TEXT NOT NULL,
  raw_memo_html TEXT NOT NULL,
  enhanced_memo_html TEXT DEFAULT NULL,
  conversations TEXT NOT NULL,
  FOREIGN KEY (user_id) REFERENCES humans(id),
  FOREIGN KEY (calendar_event_id) REFERENCES events(id)
);
