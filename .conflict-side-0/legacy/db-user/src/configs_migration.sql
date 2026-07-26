CREATE TABLE IF NOT EXISTS configs (
  id TEXT PRIMARY KEY,
  user_id TEXT NOT NULL,
  general TEXT NOT NULL,
  notification TEXT NOT NULL,
  ai TEXT NOT NULL,
  FOREIGN KEY (user_id) REFERENCES humans(id)
);
