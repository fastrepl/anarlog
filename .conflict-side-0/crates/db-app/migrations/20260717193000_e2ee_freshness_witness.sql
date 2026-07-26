CREATE TABLE IF NOT EXISTS e2ee_witness_state (
  workspace_id   TEXT PRIMARY KEY NOT NULL,
  last_sequence INTEGER NOT NULL DEFAULT 0,
  updated_at     TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  CHECK (last_sequence >= 0)
) STRICT;

CREATE TABLE IF NOT EXISTS e2ee_witness_records (
  workspace_id TEXT NOT NULL,
  record_id    TEXT NOT NULL,
  revision     INTEGER NOT NULL DEFAULT 0,
  writer_id    TEXT NOT NULL DEFAULT '',
  payload_hash TEXT NOT NULL DEFAULT '',
  payload      TEXT NOT NULL DEFAULT '',
  sequence     INTEGER NOT NULL DEFAULT 0,
  updated_at   TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  PRIMARY KEY (workspace_id, record_id),
  CHECK (revision >= 0),
  CHECK (sequence >= 0)
) STRICT;
