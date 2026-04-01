CREATE TABLE IF NOT EXISTS connections (
  id TEXT PRIMARY KEY NOT NULL,
  provider_type TEXT NOT NULL DEFAULT '',
  provider_id TEXT NOT NULL DEFAULT '',
  base_url TEXT NOT NULL DEFAULT '',
  api_key TEXT NOT NULL DEFAULT '',
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
  user_id TEXT NOT NULL DEFAULT ''
);
CREATE INDEX IF NOT EXISTS idx_connections_provider_type ON connections(provider_type);
