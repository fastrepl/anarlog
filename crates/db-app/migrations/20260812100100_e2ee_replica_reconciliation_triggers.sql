CREATE TRIGGER IF NOT EXISTS e2ee_replica_reconciliation_insert
AFTER INSERT ON e2ee_records
BEGIN
  INSERT INTO e2ee_replica_pending (record_id, workspace_id)
  VALUES (NEW.id, NEW.workspace_id)
  ON CONFLICT(record_id) DO UPDATE SET
    workspace_id = excluded.workspace_id,
    generation = e2ee_replica_pending.generation + 1;

  INSERT INTO e2ee_witness_repair_pending (record_id, workspace_id)
  VALUES (NEW.id, NEW.workspace_id)
  ON CONFLICT(record_id) DO UPDATE SET
    workspace_id = excluded.workspace_id,
    generation = e2ee_witness_repair_pending.generation + 1;
END;

CREATE TRIGGER IF NOT EXISTS e2ee_replica_reconciliation_update
AFTER UPDATE OF workspace_id, payload ON e2ee_records
BEGIN
  INSERT INTO e2ee_replica_pending (record_id, workspace_id)
  VALUES (NEW.id, NEW.workspace_id)
  ON CONFLICT(record_id) DO UPDATE SET
    workspace_id = excluded.workspace_id,
    generation = e2ee_replica_pending.generation + 1;

  INSERT INTO e2ee_witness_repair_pending (record_id, workspace_id)
  VALUES (NEW.id, NEW.workspace_id)
  ON CONFLICT(record_id) DO UPDATE SET
    workspace_id = excluded.workspace_id,
    generation = e2ee_witness_repair_pending.generation + 1;
END;

CREATE TRIGGER IF NOT EXISTS e2ee_replica_reconciliation_delete
AFTER DELETE ON e2ee_records
BEGIN
  INSERT INTO e2ee_replica_pending (record_id, workspace_id)
  VALUES (OLD.id, OLD.workspace_id)
  ON CONFLICT(record_id) DO UPDATE SET
    workspace_id = excluded.workspace_id,
    generation = e2ee_replica_pending.generation + 1;

  INSERT INTO e2ee_witness_repair_pending (record_id, workspace_id)
  VALUES (OLD.id, OLD.workspace_id)
  ON CONFLICT(record_id) DO UPDATE SET
    workspace_id = excluded.workspace_id,
    generation = e2ee_witness_repair_pending.generation + 1;
END;
