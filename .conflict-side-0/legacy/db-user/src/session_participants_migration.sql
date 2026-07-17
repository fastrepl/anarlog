CREATE TABLE IF NOT EXISTS session_participants (
  session_id TEXT NOT NULL,
  human_id TEXT NOT NULL,
  PRIMARY KEY (session_id, human_id),
  FOREIGN KEY (session_id) REFERENCES sessions(id) ON DELETE CASCADE,
  FOREIGN KEY (human_id) REFERENCES humans(id) ON DELETE CASCADE
);
