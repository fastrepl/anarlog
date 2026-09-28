-- Witness uploads leave the source device in queue order. Session metadata
-- goes first so a fresh device can list sessions before bodies arrive.
ALTER TABLE e2ee_witness_pending ADD COLUMN priority INTEGER NOT NULL DEFAULT 0;

UPDATE e2ee_witness_pending
SET priority = COALESCE((
  SELECT CASE local.table_name
    WHEN 'session_documents' THEN 1
    WHEN 'transcripts' THEN 2
    ELSE 0
  END
  FROM e2ee_local_state AS local
  WHERE local.record_id = e2ee_witness_pending.record_id
), 0);

DROP INDEX IF EXISTS idx_e2ee_witness_pending_workspace_record;
CREATE INDEX IF NOT EXISTS idx_e2ee_witness_pending_workspace_record
ON e2ee_witness_pending(workspace_id, priority, record_id);
