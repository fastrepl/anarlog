CREATE TABLE IF NOT EXISTS e2ee_witness_pending (
  record_id    TEXT PRIMARY KEY NOT NULL
               REFERENCES e2ee_local_state(record_id) ON DELETE CASCADE,
  workspace_id TEXT NOT NULL
) STRICT;

CREATE INDEX IF NOT EXISTS idx_e2ee_witness_pending_workspace_record
ON e2ee_witness_pending(workspace_id, record_id);

INSERT INTO e2ee_witness_pending (record_id, workspace_id)
SELECT local.record_id, local.workspace_id
FROM e2ee_local_state AS local
LEFT JOIN e2ee_witness_records AS witness
  ON witness.workspace_id = local.workspace_id
 AND witness.record_id = local.record_id
WHERE witness.record_id IS NULL
   OR local.revision > witness.revision
   OR (local.revision = witness.revision AND local.writer_id > witness.writer_id)
   OR (
     local.revision = witness.revision
     AND local.writer_id = witness.writer_id
     AND local.payload_hash > witness.payload_hash
   )
ON CONFLICT(record_id) DO UPDATE SET
  workspace_id = excluded.workspace_id;
