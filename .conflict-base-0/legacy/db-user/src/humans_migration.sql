CREATE TABLE IF NOT EXISTS humans (
  id TEXT PRIMARY KEY,
  organization_id TEXT DEFAULT NULL,
  is_user BOOLEAN NOT NULL,
  full_name TEXT DEFAULT NULL,
  email TEXT DEFAULT NULL,
  job_title TEXT DEFAULT NULL,
  linkedin_username TEXT DEFAULT NULL,
  FOREIGN KEY (organization_id) REFERENCES organizations (id)
);
