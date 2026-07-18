ALTER TABLE e2ee_local_state
ADD COLUMN writer_id TEXT NOT NULL DEFAULT '';

ALTER TABLE e2ee_local_state
ADD COLUMN payload TEXT NOT NULL DEFAULT '';

DELETE FROM e2ee_local_state
WHERE NOT EXISTS (
  SELECT 1
  FROM e2ee_records
  WHERE e2ee_records.id = e2ee_local_state.record_id
    AND e2ee_records.workspace_id = e2ee_local_state.workspace_id
);

UPDATE e2ee_local_state
SET payload = (
  SELECT e2ee_records.payload
  FROM e2ee_records
  WHERE e2ee_records.id = e2ee_local_state.record_id
    AND e2ee_records.workspace_id = e2ee_local_state.workspace_id
);

CREATE TABLE IF NOT EXISTS e2ee_local_device (
  id         TEXT PRIMARY KEY NOT NULL,
  writer_id  TEXT NOT NULL DEFAULT '',
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  CHECK (id = 'local')
) STRICT;
