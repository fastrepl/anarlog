-- Named folder catalog so empty folders persist after notes leave.
-- Older builds ignore this table. Not CloudSync-enabled.
-- DDL is CloudSync-safe if the table is enabled later.
CREATE TABLE IF NOT EXISTS folders (
  id            TEXT PRIMARY KEY NOT NULL,
  workspace_id  TEXT NOT NULL DEFAULT '',
  path          TEXT NOT NULL DEFAULT '',
  created_at    TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  updated_at    TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  deleted_at    TEXT
) STRICT;

CREATE INDEX IF NOT EXISTS idx_folders_path
  ON folders(path);
