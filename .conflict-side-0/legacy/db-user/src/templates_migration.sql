CREATE TABLE IF NOT EXISTS templates (
  id TEXT PRIMARY KEY,
  user_id TEXT NOT NULL,
  title TEXT NOT NULL,
  description TEXT NOT NULL,
  sections TEXT NOT NULL,
  tags TEXT NOT NULL,
  FOREIGN KEY (user_id) REFERENCES humans(id)
);
