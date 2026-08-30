-- Local folder materials (syllabus, PDF, text) keyed by folder_path.
-- Older builds ignore this table. Not CloudSync-enabled.
-- DDL is CloudSync-safe if the table is enabled later.
CREATE TABLE IF NOT EXISTS folder_attachments (
  id                TEXT PRIMARY KEY NOT NULL,
  workspace_id      TEXT NOT NULL DEFAULT '',
  folder_path       TEXT NOT NULL DEFAULT '',
  filename          TEXT NOT NULL DEFAULT '',
  relative_path     TEXT NOT NULL DEFAULT '',
  content_type      TEXT NOT NULL DEFAULT '',
  size_bytes        INTEGER NOT NULL DEFAULT 0,
  sha256            TEXT NOT NULL DEFAULT '',
  storage_kind      TEXT NOT NULL DEFAULT 'local_file',
  cloud_object_key  TEXT NOT NULL DEFAULT '',
  source_type       TEXT NOT NULL DEFAULT 'folder_material',
  source_id         TEXT NOT NULL DEFAULT '',
  metadata_json     TEXT NOT NULL DEFAULT '{}',
  created_at        TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  updated_at        TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  deleted_at        TEXT
) STRICT;

CREATE INDEX IF NOT EXISTS idx_folder_attachments_folder_path
  ON folder_attachments(folder_path);
