CREATE TABLE e2ee_cloud_authority (
  workspace_id TEXT PRIMARY KEY NOT NULL,
  after_sequence INTEGER CHECK (after_sequence >= 0),
  pull_in_progress INTEGER NOT NULL DEFAULT 0
) STRICT;

CREATE TABLE e2ee_cloud_outbox (
  workspace_id TEXT NOT NULL,
  record_id TEXT NOT NULL,
  table_name TEXT NOT NULL,
  row_id TEXT NOT NULL,
  payload_hash TEXT NOT NULL,
  payload TEXT NOT NULL,
  base_payload_hash TEXT,
  base_payload TEXT,
  PRIMARY KEY (workspace_id, record_id)
) STRICT;
CREATE INDEX idx_e2ee_cloud_outbox_row ON e2ee_cloud_outbox(workspace_id, table_name, row_id);

CREATE TABLE e2ee_cloud_batches (
  workspace_id TEXT PRIMARY KEY NOT NULL,
  mutation_id TEXT NOT NULL,
  base_sequence INTEGER NOT NULL,
  initialize INTEGER NOT NULL,
  events_json TEXT NOT NULL
) STRICT;
