CREATE TABLE daily_notes (
  id TEXT PRIMARY KEY NOT NULL,
  date TEXT NOT NULL,
  body TEXT NOT NULL,
  user_id TEXT NOT NULL
);
CREATE TABLE daily_summaries (
  id TEXT PRIMARY KEY NOT NULL,
  daily_note_id TEXT NOT NULL,
  date TEXT NOT NULL,
  content TEXT NOT NULL,
  timeline_json TEXT NOT NULL,
  topics_json TEXT NOT NULL,
  status TEXT NOT NULL,
  source_cursor_ms INTEGER NOT NULL,
  source_fingerprint TEXT NOT NULL,
  generation_error TEXT NOT NULL,
  generated_at TEXT NOT NULL
);
