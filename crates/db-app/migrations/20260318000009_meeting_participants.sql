CREATE TABLE IF NOT EXISTS meeting_participants (
  id TEXT PRIMARY KEY NOT NULL,
  meeting_id TEXT NOT NULL DEFAULT '' REFERENCES meetings(id),
  human_id TEXT NOT NULL DEFAULT '' REFERENCES humans(id),
  source TEXT NOT NULL DEFAULT 'manual',
  user_id TEXT NOT NULL DEFAULT ''
);
CREATE INDEX IF NOT EXISTS idx_mp_meeting ON meeting_participants(meeting_id);
CREATE INDEX IF NOT EXISTS idx_mp_human ON meeting_participants(human_id);
CREATE UNIQUE INDEX IF NOT EXISTS idx_mp_meeting_human ON meeting_participants(meeting_id, human_id);
