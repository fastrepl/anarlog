CREATE TABLE IF NOT EXISTS task_words (
  id TEXT PRIMARY KEY NOT NULL,
  task_id TEXT NOT NULL DEFAULT '' REFERENCES tasks(id),
  text TEXT NOT NULL DEFAULT '',
  start_ms INTEGER NOT NULL DEFAULT 0,
  end_ms INTEGER NOT NULL DEFAULT 0,
  channel INTEGER NOT NULL DEFAULT 0,
  state TEXT NOT NULL DEFAULT 'final',
  user_id TEXT NOT NULL DEFAULT '',
  visibility TEXT NOT NULL DEFAULT 'public'
);
CREATE INDEX IF NOT EXISTS idx_task_words_task ON task_words(task_id, start_ms);

CREATE TABLE IF NOT EXISTS task_speaker_hints (
  id TEXT PRIMARY KEY NOT NULL,
  task_id TEXT NOT NULL DEFAULT '' REFERENCES tasks(id),
  word_id TEXT NOT NULL DEFAULT '' REFERENCES task_words(id),
  kind TEXT NOT NULL DEFAULT '',
  speaker_index INTEGER,
  provider TEXT,
  channel INTEGER,
  human_id TEXT,
  user_id TEXT NOT NULL DEFAULT '',
  visibility TEXT NOT NULL DEFAULT 'public'
);
CREATE INDEX IF NOT EXISTS idx_task_hints_task ON task_speaker_hints(task_id);
CREATE INDEX IF NOT EXISTS idx_task_hints_word ON task_speaker_hints(word_id);
