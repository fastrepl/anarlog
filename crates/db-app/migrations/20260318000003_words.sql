CREATE TABLE IF NOT EXISTS words (
  id TEXT PRIMARY KEY NOT NULL,
  meeting_id TEXT NOT NULL DEFAULT '' REFERENCES meetings(id),
  text TEXT NOT NULL DEFAULT '',
  start_ms INTEGER NOT NULL DEFAULT 0,
  end_ms INTEGER NOT NULL DEFAULT 0,
  channel INTEGER NOT NULL DEFAULT 0,
  state TEXT NOT NULL DEFAULT 'final',
  user_id TEXT NOT NULL DEFAULT '',
  visibility TEXT NOT NULL DEFAULT 'public'
);
CREATE INDEX IF NOT EXISTS idx_words_meeting ON words(meeting_id, start_ms);
