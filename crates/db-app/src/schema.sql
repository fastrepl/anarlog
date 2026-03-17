CREATE TABLE IF NOT EXISTS sessions (
  id TEXT PRIMARY KEY NOT NULL,
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
  title TEXT,
  summary TEXT,
  memo TEXT
);
CREATE TABLE IF NOT EXISTS words (
  id TEXT PRIMARY KEY NOT NULL,
  session_id TEXT NOT NULL REFERENCES sessions(id),
  text TEXT NOT NULL,
  start_ms INTEGER NOT NULL,
  end_ms INTEGER NOT NULL,
  channel INTEGER NOT NULL,
  state TEXT NOT NULL DEFAULT 'final'
);
CREATE INDEX IF NOT EXISTS idx_words_session ON words(session_id, start_ms);
CREATE TABLE IF NOT EXISTS speaker_hints (
  id TEXT PRIMARY KEY NOT NULL,
  session_id TEXT NOT NULL REFERENCES sessions(id),
  word_id TEXT NOT NULL REFERENCES words(id),
  kind TEXT NOT NULL,
  speaker_index INTEGER,
  provider TEXT,
  channel INTEGER,
  human_id TEXT
);
CREATE INDEX IF NOT EXISTS idx_hints_session ON speaker_hints(session_id);
CREATE INDEX IF NOT EXISTS idx_hints_word ON speaker_hints(word_id);
