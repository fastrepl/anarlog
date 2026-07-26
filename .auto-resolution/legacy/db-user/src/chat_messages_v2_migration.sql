CREATE TABLE IF NOT EXISTS chat_messages_v2 (
  id TEXT PRIMARY KEY,
  conversation_id TEXT NOT NULL,
  role TEXT CHECK(role IN ('system', 'user', 'assistant')) NOT NULL,
  parts TEXT NOT NULL,
  -- JSON string of message parts array
  metadata TEXT,
  -- JSON string for mentions, selections, etc.
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  FOREIGN KEY (conversation_id) REFERENCES chat_conversations(id) ON DELETE CASCADE
);
