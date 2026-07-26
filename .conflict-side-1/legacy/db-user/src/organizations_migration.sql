CREATE TABLE IF NOT EXISTS organizations (
  id TEXT PRIMARY KEY,
  name TEXT NOT NULL,
  linkedin_url TEXT DEFAULT NULL,
  website_url TEXT DEFAULT NULL,
  description TEXT DEFAULT NULL
);
