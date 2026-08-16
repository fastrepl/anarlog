CREATE TABLE IF NOT EXISTS e2ee_replica_payload_hashes (
  record_id    TEXT PRIMARY KEY NOT NULL,
  workspace_id TEXT NOT NULL DEFAULT '',
  payload_hash TEXT NOT NULL DEFAULT '',
  updated_at   TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
) STRICT;

INSERT INTO e2ee_replica_payload_hashes (record_id, workspace_id, payload_hash)
SELECT id, workspace_id, payload_hash
FROM e2ee_records
WHERE true
ON CONFLICT(record_id) DO UPDATE SET
  workspace_id = excluded.workspace_id,
  payload_hash = excluded.payload_hash,
  updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now');
