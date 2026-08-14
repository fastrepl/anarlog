CREATE TABLE IF NOT EXISTS enterprise_session_delivery_state (
  server_url TEXT NOT NULL CHECK (server_url <> ''),
  workspace_id TEXT NOT NULL CHECK (workspace_id <> ''),
  consumer_id TEXT NOT NULL CHECK (consumer_id <> ''),
  cursor INTEGER NOT NULL DEFAULT 0 CHECK (cursor >= 0),
  updated_at TEXT NOT NULL,
  PRIMARY KEY (server_url, workspace_id, consumer_id)
);

CREATE TABLE IF NOT EXISTS enterprise_session_delivery_receipts (
  server_url TEXT NOT NULL,
  workspace_id TEXT NOT NULL,
  consumer_id TEXT NOT NULL,
  job_id TEXT NOT NULL CHECK (job_id <> ''),
  revision INTEGER NOT NULL CHECK (revision > 0),
  content_hash TEXT NOT NULL CHECK (length(content_hash) = 64),
  acknowledged_at TEXT,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  PRIMARY KEY (server_url, workspace_id, consumer_id, job_id, revision),
  FOREIGN KEY (server_url, workspace_id, consumer_id)
    REFERENCES enterprise_session_delivery_state (server_url, workspace_id, consumer_id)
    ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_enterprise_session_delivery_pending_acks
ON enterprise_session_delivery_receipts (
  server_url,
  workspace_id,
  consumer_id,
  acknowledged_at,
  revision
);

CREATE TABLE IF NOT EXISTS enterprise_session_completion_outbox (
  source_id TEXT PRIMARY KEY CHECK (source_id <> ''),
  workspace_id TEXT NOT NULL CHECK (workspace_id <> ''),
  session_id TEXT NOT NULL CHECK (session_id <> ''),
  revision INTEGER NOT NULL CHECK (revision > 0),
  created_at TEXT NOT NULL,
  dispatched_at TEXT
);

CREATE INDEX IF NOT EXISTS idx_enterprise_session_completion_pending
ON enterprise_session_completion_outbox (dispatched_at, created_at, source_id);
