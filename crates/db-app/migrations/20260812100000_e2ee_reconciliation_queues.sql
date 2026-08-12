CREATE TABLE IF NOT EXISTS e2ee_replica_pending (
  record_id    TEXT PRIMARY KEY NOT NULL,
  workspace_id TEXT NOT NULL,
  generation   INTEGER NOT NULL DEFAULT 1
) STRICT;

CREATE INDEX IF NOT EXISTS idx_e2ee_replica_pending_workspace_record
ON e2ee_replica_pending(workspace_id, record_id);

CREATE TABLE IF NOT EXISTS e2ee_witness_repair_pending (
  record_id    TEXT PRIMARY KEY NOT NULL,
  workspace_id TEXT NOT NULL,
  generation   INTEGER NOT NULL DEFAULT 1
) STRICT;

CREATE INDEX IF NOT EXISTS idx_e2ee_witness_repair_pending_workspace_record
ON e2ee_witness_repair_pending(workspace_id, record_id);

CREATE TRIGGER IF NOT EXISTS e2ee_witness_reconciliation_insert
AFTER INSERT ON e2ee_witness_records
BEGIN
  INSERT INTO e2ee_replica_pending (record_id, workspace_id)
  VALUES (NEW.record_id, NEW.workspace_id)
  ON CONFLICT(record_id) DO UPDATE SET
    workspace_id = excluded.workspace_id,
    generation = e2ee_replica_pending.generation + 1;

  INSERT INTO e2ee_witness_repair_pending (record_id, workspace_id)
  VALUES (NEW.record_id, NEW.workspace_id)
  ON CONFLICT(record_id) DO UPDATE SET
    workspace_id = excluded.workspace_id,
    generation = e2ee_witness_repair_pending.generation + 1;
END;

CREATE TRIGGER IF NOT EXISTS e2ee_witness_reconciliation_update
AFTER UPDATE OF workspace_id, revision, writer_id, payload_hash, payload ON e2ee_witness_records
BEGIN
  INSERT INTO e2ee_replica_pending (record_id, workspace_id)
  VALUES (NEW.record_id, NEW.workspace_id)
  ON CONFLICT(record_id) DO UPDATE SET
    workspace_id = excluded.workspace_id,
    generation = e2ee_replica_pending.generation + 1;

  INSERT INTO e2ee_witness_repair_pending (record_id, workspace_id)
  VALUES (NEW.record_id, NEW.workspace_id)
  ON CONFLICT(record_id) DO UPDATE SET
    workspace_id = excluded.workspace_id,
    generation = e2ee_witness_repair_pending.generation + 1;
END;

CREATE TRIGGER IF NOT EXISTS e2ee_witness_reconciliation_delete
AFTER DELETE ON e2ee_witness_records
BEGIN
  INSERT INTO e2ee_replica_pending (record_id, workspace_id)
  VALUES (OLD.record_id, OLD.workspace_id)
  ON CONFLICT(record_id) DO UPDATE SET
    workspace_id = excluded.workspace_id,
    generation = e2ee_replica_pending.generation + 1;

  INSERT INTO e2ee_witness_repair_pending (record_id, workspace_id)
  VALUES (OLD.record_id, OLD.workspace_id)
  ON CONFLICT(record_id) DO UPDATE SET
    workspace_id = excluded.workspace_id,
    generation = e2ee_witness_repair_pending.generation + 1;
END;

INSERT INTO e2ee_replica_pending (record_id, workspace_id)
SELECT id, workspace_id FROM e2ee_records
WHERE true
ON CONFLICT(record_id) DO UPDATE SET
  workspace_id = excluded.workspace_id;

INSERT INTO e2ee_replica_pending (record_id, workspace_id)
SELECT record_id, workspace_id FROM e2ee_witness_records
WHERE true
ON CONFLICT(record_id) DO UPDATE SET
  workspace_id = excluded.workspace_id;

INSERT INTO e2ee_witness_repair_pending (record_id, workspace_id)
SELECT record_id, workspace_id FROM e2ee_witness_records
WHERE true
ON CONFLICT(record_id) DO UPDATE SET
  workspace_id = excluded.workspace_id;
