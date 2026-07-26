CREATE TABLE IF NOT EXISTS chat_groups (
  id TEXT PRIMARY KEY,
  user_id TEXT NOT NULL,
  created_at TEXT NOT NULL,
  name TEXT DEFAULT NULL,
  FOREIGN KEY (user_id) REFERENCES humans(id)
);
