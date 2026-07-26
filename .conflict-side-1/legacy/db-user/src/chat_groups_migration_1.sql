ALTER TABLE
  chat_groups
ADD
  COLUMN session_id TEXT NOT NULL REFERENCES sessions(id);
