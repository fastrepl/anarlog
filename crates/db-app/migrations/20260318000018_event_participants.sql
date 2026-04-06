CREATE TABLE IF NOT EXISTS event_participants (
  id TEXT PRIMARY KEY NOT NULL,
  event_id TEXT NOT NULL DEFAULT '' REFERENCES events(id) ON DELETE CASCADE,
  human_id TEXT REFERENCES humans(id) ON DELETE SET NULL,
  email TEXT NOT NULL DEFAULT '',
  name TEXT NOT NULL DEFAULT '',
  is_organizer INTEGER NOT NULL DEFAULT 0,
  is_current_user INTEGER NOT NULL DEFAULT 0,
  user_id TEXT NOT NULL DEFAULT ''
);
CREATE INDEX IF NOT EXISTS idx_ep_event ON event_participants(event_id);
CREATE INDEX IF NOT EXISTS idx_ep_human ON event_participants(human_id);
CREATE UNIQUE INDEX IF NOT EXISTS idx_ep_event_email ON event_participants(event_id, email);
