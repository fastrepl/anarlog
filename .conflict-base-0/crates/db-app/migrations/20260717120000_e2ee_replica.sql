CREATE TABLE IF NOT EXISTS e2ee_records (
  id            TEXT PRIMARY KEY NOT NULL,
  workspace_id  TEXT NOT NULL DEFAULT '',
  payload       TEXT NOT NULL DEFAULT '',
  created_at    TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  updated_at    TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
) STRICT;

CREATE INDEX IF NOT EXISTS idx_e2ee_records_workspace
ON e2ee_records(workspace_id, id);

CREATE TABLE IF NOT EXISTS e2ee_local_state (
  record_id      TEXT PRIMARY KEY NOT NULL,
  workspace_id   TEXT NOT NULL DEFAULT '',
  table_name     TEXT NOT NULL DEFAULT '',
  row_id         TEXT NOT NULL DEFAULT '',
  field_name     TEXT NOT NULL DEFAULT '',
  revision       INTEGER NOT NULL DEFAULT 0,
  value_tag      TEXT NOT NULL DEFAULT '',
  payload_hash   TEXT NOT NULL DEFAULT '',
  updated_at     TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
) STRICT;

CREATE INDEX IF NOT EXISTS idx_e2ee_local_state_row
ON e2ee_local_state(workspace_id, table_name, row_id);
