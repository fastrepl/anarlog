CREATE TABLE IF NOT EXISTS e2ee_ciphertext_archive (
  workspace_id TEXT NOT NULL DEFAULT '',
  record_id    TEXT NOT NULL DEFAULT '',
  payload_hash TEXT NOT NULL DEFAULT '',
  payload      TEXT NOT NULL DEFAULT '',
  created_at   TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  PRIMARY KEY (workspace_id, record_id, payload_hash),
  CHECK (workspace_id <> ''),
  CHECK (record_id <> ''),
  CHECK (payload_hash <> ''),
  CHECK (payload <> '')
) STRICT, WITHOUT ROWID;

UPDATE e2ee_records AS replica
SET payload_hash = COALESCE(
  (
    SELECT local.payload_hash
    FROM e2ee_local_state AS local
    WHERE local.record_id = replica.id
      AND local.workspace_id = replica.workspace_id
      AND local.payload = replica.payload
      AND local.payload_hash <> ''
    LIMIT 1
  ),
  (
    SELECT witness.payload_hash
    FROM e2ee_witness_records AS witness
    WHERE witness.record_id = replica.id
      AND witness.workspace_id = replica.workspace_id
      AND witness.payload = replica.payload
      AND witness.payload_hash <> ''
    LIMIT 1
  ),
  ''
)
WHERE replica.payload_hash = '';

INSERT OR IGNORE INTO e2ee_ciphertext_archive (
  workspace_id, record_id, payload_hash, payload
)
SELECT local.workspace_id, local.record_id, local.payload_hash, local.payload
FROM e2ee_local_state AS local
LEFT JOIN e2ee_records AS replica
  ON replica.id = local.record_id
 AND replica.workspace_id = local.workspace_id
 AND replica.payload_hash = local.payload_hash
 AND replica.payload = local.payload
WHERE local.workspace_id <> ''
  AND local.record_id <> ''
  AND local.payload_hash <> ''
  AND local.payload <> ''
  AND replica.id IS NULL;

INSERT OR IGNORE INTO e2ee_ciphertext_archive (
  workspace_id, record_id, payload_hash, payload
)
SELECT witness.workspace_id, witness.record_id, witness.payload_hash, witness.payload
FROM e2ee_witness_records AS witness
LEFT JOIN e2ee_records AS replica
  ON replica.id = witness.record_id
 AND replica.workspace_id = witness.workspace_id
 AND replica.payload_hash = witness.payload_hash
 AND replica.payload = witness.payload
WHERE witness.workspace_id <> ''
  AND witness.record_id <> ''
  AND witness.payload_hash <> ''
  AND witness.payload <> ''
  AND replica.id IS NULL;

UPDATE e2ee_local_state SET payload = '' WHERE payload <> '';
UPDATE e2ee_witness_records SET payload = '' WHERE payload <> '';

CREATE VIEW IF NOT EXISTS e2ee_local_state_resolved AS
SELECT
  local.record_id,
  local.workspace_id,
  local.table_name,
  local.row_id,
  local.field_name,
  local.revision,
  local.writer_id,
  local.value_tag,
  local.payload_hash,
  CASE
    WHEN replica.workspace_id = local.workspace_id
      AND replica.payload_hash = local.payload_hash
      THEN replica.payload
    ELSE archive.payload
  END AS payload,
  local.updated_at
FROM e2ee_local_state AS local
LEFT JOIN e2ee_records AS replica
  ON replica.id = local.record_id
LEFT JOIN e2ee_ciphertext_archive AS archive
  ON archive.workspace_id = local.workspace_id
 AND archive.record_id = local.record_id
 AND archive.payload_hash = local.payload_hash;

CREATE VIEW IF NOT EXISTS e2ee_witness_records_resolved AS
SELECT
  witness.workspace_id,
  witness.record_id,
  witness.revision,
  witness.writer_id,
  witness.payload_hash,
  CASE
    WHEN replica.workspace_id = witness.workspace_id
      AND replica.payload_hash = witness.payload_hash
      THEN replica.payload
    ELSE archive.payload
  END AS payload,
  witness.sequence,
  witness.updated_at
FROM e2ee_witness_records AS witness
LEFT JOIN e2ee_records AS replica
  ON replica.id = witness.record_id
LEFT JOIN e2ee_ciphertext_archive AS archive
  ON archive.workspace_id = witness.workspace_id
 AND archive.record_id = witness.record_id
 AND archive.payload_hash = witness.payload_hash;

CREATE TRIGGER IF NOT EXISTS e2ee_records_archive_before_update
BEFORE UPDATE OF workspace_id, payload_hash, payload ON e2ee_records
WHEN OLD.payload <> ''
  AND OLD.payload_hash <> ''
  AND (
    OLD.workspace_id <> NEW.workspace_id
    OR OLD.payload_hash <> NEW.payload_hash
    OR OLD.payload <> NEW.payload
  )
  AND (
    EXISTS(
      SELECT 1 FROM e2ee_local_state AS local
      WHERE local.workspace_id = OLD.workspace_id
        AND local.record_id = OLD.id
        AND local.payload_hash = OLD.payload_hash
    )
    OR EXISTS(
      SELECT 1 FROM e2ee_witness_records AS witness
      WHERE witness.workspace_id = OLD.workspace_id
        AND witness.record_id = OLD.id
        AND witness.payload_hash = OLD.payload_hash
    )
  )
BEGIN
  INSERT INTO e2ee_ciphertext_archive (
    workspace_id, record_id, payload_hash, payload
  )
  SELECT OLD.workspace_id, OLD.id, OLD.payload_hash, OLD.payload
  WHERE NOT EXISTS (
    SELECT 1 FROM e2ee_ciphertext_archive AS archive
    WHERE archive.workspace_id = OLD.workspace_id
      AND archive.record_id = OLD.id
      AND archive.payload_hash = OLD.payload_hash
  );
END;

CREATE TRIGGER IF NOT EXISTS e2ee_records_archive_before_delete
BEFORE DELETE ON e2ee_records
WHEN OLD.payload <> ''
  AND OLD.payload_hash <> ''
  AND (
    EXISTS(
      SELECT 1 FROM e2ee_local_state AS local
      WHERE local.workspace_id = OLD.workspace_id
        AND local.record_id = OLD.id
        AND local.payload_hash = OLD.payload_hash
    )
    OR EXISTS(
      SELECT 1 FROM e2ee_witness_records AS witness
      WHERE witness.workspace_id = OLD.workspace_id
        AND witness.record_id = OLD.id
        AND witness.payload_hash = OLD.payload_hash
    )
  )
BEGIN
  INSERT INTO e2ee_ciphertext_archive (
    workspace_id, record_id, payload_hash, payload
  )
  SELECT OLD.workspace_id, OLD.id, OLD.payload_hash, OLD.payload
  WHERE NOT EXISTS (
    SELECT 1 FROM e2ee_ciphertext_archive AS archive
    WHERE archive.workspace_id = OLD.workspace_id
      AND archive.record_id = OLD.id
      AND archive.payload_hash = OLD.payload_hash
  );
END;

CREATE TRIGGER IF NOT EXISTS e2ee_records_clear_stale_payload_hash
AFTER UPDATE OF payload ON e2ee_records
WHEN OLD.payload <> NEW.payload
  AND NEW.payload_hash = OLD.payload_hash
BEGIN
  UPDATE e2ee_records
  SET payload_hash = ''
  WHERE id = NEW.id
    AND workspace_id = NEW.workspace_id
    AND payload = NEW.payload
    AND payload_hash = NEW.payload_hash;
END;

CREATE TRIGGER IF NOT EXISTS e2ee_local_state_normalize_payload_insert
AFTER INSERT ON e2ee_local_state
WHEN NEW.payload <> ''
BEGIN
  INSERT INTO e2ee_ciphertext_archive (
    workspace_id, record_id, payload_hash, payload
  )
  SELECT NEW.workspace_id, NEW.record_id, NEW.payload_hash, NEW.payload
  WHERE NOT EXISTS (
    SELECT 1 FROM e2ee_records AS replica
    WHERE replica.id = NEW.record_id
      AND replica.workspace_id = NEW.workspace_id
      AND replica.payload_hash = NEW.payload_hash
      AND replica.payload = NEW.payload
  )
  AND NOT EXISTS (
    SELECT 1 FROM e2ee_ciphertext_archive AS archive
    WHERE archive.workspace_id = NEW.workspace_id
      AND archive.record_id = NEW.record_id
      AND archive.payload_hash = NEW.payload_hash
  );
  UPDATE e2ee_local_state SET payload = '' WHERE record_id = NEW.record_id;
END;

CREATE TRIGGER IF NOT EXISTS e2ee_local_state_normalize_payload_update
AFTER UPDATE OF payload ON e2ee_local_state
WHEN NEW.payload <> ''
BEGIN
  INSERT INTO e2ee_ciphertext_archive (
    workspace_id, record_id, payload_hash, payload
  )
  SELECT NEW.workspace_id, NEW.record_id, NEW.payload_hash, NEW.payload
  WHERE NOT EXISTS (
    SELECT 1 FROM e2ee_records AS replica
    WHERE replica.id = NEW.record_id
      AND replica.workspace_id = NEW.workspace_id
      AND replica.payload_hash = NEW.payload_hash
      AND replica.payload = NEW.payload
  )
  AND NOT EXISTS (
    SELECT 1 FROM e2ee_ciphertext_archive AS archive
    WHERE archive.workspace_id = NEW.workspace_id
      AND archive.record_id = NEW.record_id
      AND archive.payload_hash = NEW.payload_hash
  );
  UPDATE e2ee_local_state SET payload = '' WHERE record_id = NEW.record_id;
END;

CREATE TRIGGER IF NOT EXISTS e2ee_witness_records_normalize_payload_insert
AFTER INSERT ON e2ee_witness_records
WHEN NEW.payload <> ''
BEGIN
  INSERT INTO e2ee_ciphertext_archive (
    workspace_id, record_id, payload_hash, payload
  )
  SELECT NEW.workspace_id, NEW.record_id, NEW.payload_hash, NEW.payload
  WHERE NOT EXISTS (
    SELECT 1 FROM e2ee_records AS replica
    WHERE replica.id = NEW.record_id
      AND replica.workspace_id = NEW.workspace_id
      AND replica.payload_hash = NEW.payload_hash
      AND replica.payload = NEW.payload
  )
  AND NOT EXISTS (
    SELECT 1 FROM e2ee_ciphertext_archive AS archive
    WHERE archive.workspace_id = NEW.workspace_id
      AND archive.record_id = NEW.record_id
      AND archive.payload_hash = NEW.payload_hash
  );
  UPDATE e2ee_witness_records
  SET payload = ''
  WHERE workspace_id = NEW.workspace_id AND record_id = NEW.record_id;
END;

CREATE TRIGGER IF NOT EXISTS e2ee_witness_records_normalize_payload_update
AFTER UPDATE OF payload ON e2ee_witness_records
WHEN NEW.payload <> ''
BEGIN
  INSERT INTO e2ee_ciphertext_archive (
    workspace_id, record_id, payload_hash, payload
  )
  SELECT NEW.workspace_id, NEW.record_id, NEW.payload_hash, NEW.payload
  WHERE NOT EXISTS (
    SELECT 1 FROM e2ee_records AS replica
    WHERE replica.id = NEW.record_id
      AND replica.workspace_id = NEW.workspace_id
      AND replica.payload_hash = NEW.payload_hash
      AND replica.payload = NEW.payload
  )
  AND NOT EXISTS (
    SELECT 1 FROM e2ee_ciphertext_archive AS archive
    WHERE archive.workspace_id = NEW.workspace_id
      AND archive.record_id = NEW.record_id
      AND archive.payload_hash = NEW.payload_hash
  );
  UPDATE e2ee_witness_records
  SET payload = ''
  WHERE workspace_id = NEW.workspace_id AND record_id = NEW.record_id;
END;

CREATE TRIGGER IF NOT EXISTS e2ee_local_state_prune_archive_delete
AFTER DELETE ON e2ee_local_state
BEGIN
  DELETE FROM e2ee_ciphertext_archive
  WHERE workspace_id = OLD.workspace_id
    AND record_id = OLD.record_id
    AND payload_hash = OLD.payload_hash
    AND NOT EXISTS (
      SELECT 1 FROM e2ee_witness_records AS witness
      WHERE witness.workspace_id = OLD.workspace_id
        AND witness.record_id = OLD.record_id
        AND witness.payload_hash = OLD.payload_hash
    );
END;

CREATE TRIGGER IF NOT EXISTS e2ee_local_state_prune_archive_update
AFTER UPDATE OF workspace_id, payload_hash ON e2ee_local_state
WHEN OLD.workspace_id <> NEW.workspace_id OR OLD.payload_hash <> NEW.payload_hash
BEGIN
  DELETE FROM e2ee_ciphertext_archive
  WHERE workspace_id = OLD.workspace_id
    AND record_id = OLD.record_id
    AND payload_hash = OLD.payload_hash
    AND NOT EXISTS (
      SELECT 1 FROM e2ee_witness_records AS witness
      WHERE witness.workspace_id = OLD.workspace_id
        AND witness.record_id = OLD.record_id
        AND witness.payload_hash = OLD.payload_hash
    );
END;

CREATE TRIGGER IF NOT EXISTS e2ee_witness_records_prune_archive_delete
AFTER DELETE ON e2ee_witness_records
BEGIN
  DELETE FROM e2ee_ciphertext_archive
  WHERE workspace_id = OLD.workspace_id
    AND record_id = OLD.record_id
    AND payload_hash = OLD.payload_hash
    AND NOT EXISTS (
      SELECT 1 FROM e2ee_local_state AS local
      WHERE local.workspace_id = OLD.workspace_id
        AND local.record_id = OLD.record_id
        AND local.payload_hash = OLD.payload_hash
    );
END;

CREATE TRIGGER IF NOT EXISTS e2ee_witness_records_prune_archive_update
AFTER UPDATE OF workspace_id, payload_hash ON e2ee_witness_records
WHEN OLD.workspace_id <> NEW.workspace_id OR OLD.payload_hash <> NEW.payload_hash
BEGIN
  DELETE FROM e2ee_ciphertext_archive
  WHERE workspace_id = OLD.workspace_id
    AND record_id = OLD.record_id
    AND payload_hash = OLD.payload_hash
    AND NOT EXISTS (
      SELECT 1 FROM e2ee_local_state AS local
      WHERE local.workspace_id = OLD.workspace_id
        AND local.record_id = OLD.record_id
        AND local.payload_hash = OLD.payload_hash
    );
END;
