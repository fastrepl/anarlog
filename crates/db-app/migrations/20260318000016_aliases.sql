CREATE TABLE IF NOT EXISTS aliases (
  id           TEXT PRIMARY KEY NOT NULL,
  human_id     TEXT NOT NULL DEFAULT '' REFERENCES humans(id),
  provider     TEXT NOT NULL DEFAULT '',
  external_id  TEXT NOT NULL DEFAULT '',
  workspace_id TEXT NOT NULL DEFAULT '',
  display_name TEXT NOT NULL DEFAULT '',
  confidence   TEXT NOT NULL DEFAULT 'confirmed',
  created_at   TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
  user_id      TEXT NOT NULL DEFAULT ''
);
CREATE UNIQUE INDEX IF NOT EXISTS idx_aliases_lookup ON aliases(provider, external_id, workspace_id);
CREATE INDEX IF NOT EXISTS idx_aliases_human ON aliases(human_id);
