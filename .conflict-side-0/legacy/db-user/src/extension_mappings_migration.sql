CREATE TABLE IF NOT EXISTS extension_mappings (
  id TEXT PRIMARY KEY,
  extension_id TEXT NOT NULL,
  user_id TEXT NOT NULL,
  config TEXT DEFAULT '{}' NOT NULL,
  position TEXT DEFAULT NULL,
  FOREIGN KEY (user_id) REFERENCES humans(id)
);
