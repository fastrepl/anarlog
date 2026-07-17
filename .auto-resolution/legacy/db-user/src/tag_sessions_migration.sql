CREATE TABLE IF NOT EXISTS tags_sessions (
  tag_id TEXT NOT NULL,
  session_id TEXT NOT NULL,
  PRIMARY KEY (tag_id, session_id),
  FOREIGN KEY (tag_id) REFERENCES tags(id),
  FOREIGN KEY (session_id) REFERENCES sessions(id) ON DELETE CASCADE
);
