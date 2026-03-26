CREATE TABLE IF NOT EXISTS speaker_hints (
  id TEXT PRIMARY KEY NOT NULL,
  meeting_id TEXT NOT NULL DEFAULT '' REFERENCES meetings(id),
  word_id TEXT NOT NULL DEFAULT '' REFERENCES words(id),
  kind TEXT NOT NULL DEFAULT '',
  speaker_index INTEGER,
  provider TEXT,
  channel INTEGER,
  human_id TEXT,
  user_id TEXT NOT NULL DEFAULT '',
  visibility TEXT NOT NULL DEFAULT 'public',
  task_id TEXT DEFAULT NULL REFERENCES tasks(id)
);
CREATE INDEX IF NOT EXISTS idx_hints_meeting ON speaker_hints(meeting_id);
CREATE INDEX IF NOT EXISTS idx_hints_word ON speaker_hints(word_id);
CREATE INDEX IF NOT EXISTS idx_hints_task ON speaker_hints(task_id);
