DROP VIEW IF EXISTS e2ee_local_state_resolved;
DROP VIEW IF EXISTS e2ee_witness_records_resolved;

DROP TRIGGER IF EXISTS e2ee_records_archive_before_update;
DROP TRIGGER IF EXISTS e2ee_records_archive_before_delete;
DROP TRIGGER IF EXISTS e2ee_records_clear_stale_payload_hash;
DROP TRIGGER IF EXISTS e2ee_local_state_normalize_payload_insert;
DROP TRIGGER IF EXISTS e2ee_local_state_normalize_payload_update;
DROP TRIGGER IF EXISTS e2ee_witness_records_normalize_payload_insert;
DROP TRIGGER IF EXISTS e2ee_witness_records_normalize_payload_update;
DROP TRIGGER IF EXISTS e2ee_local_state_prune_archive_delete;
DROP TRIGGER IF EXISTS e2ee_local_state_prune_archive_update;
DROP TRIGGER IF EXISTS e2ee_witness_records_prune_archive_delete;
DROP TRIGGER IF EXISTS e2ee_witness_records_prune_archive_update;

ALTER TABLE e2ee_records DROP COLUMN payload_hash;

CREATE VIEW e2ee_local_state_resolved AS
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
      AND replica_hash.payload_hash = local.payload_hash
      THEN replica.payload
    ELSE archive.payload
  END AS payload,
  local.updated_at
FROM e2ee_local_state AS local
LEFT JOIN e2ee_records AS replica
  ON replica.id = local.record_id
LEFT JOIN e2ee_replica_payload_hashes AS replica_hash
  ON replica_hash.record_id = replica.id
 AND replica_hash.workspace_id = replica.workspace_id
LEFT JOIN e2ee_ciphertext_archive AS archive
  ON archive.workspace_id = local.workspace_id
 AND archive.record_id = local.record_id
 AND archive.payload_hash = local.payload_hash;

CREATE VIEW e2ee_witness_records_resolved AS
SELECT
  witness.workspace_id,
  witness.record_id,
  witness.revision,
  witness.writer_id,
  witness.payload_hash,
  CASE
    WHEN replica.workspace_id = witness.workspace_id
      AND replica_hash.payload_hash = witness.payload_hash
      THEN replica.payload
    ELSE archive.payload
  END AS payload,
  witness.sequence,
  witness.updated_at
FROM e2ee_witness_records AS witness
LEFT JOIN e2ee_records AS replica
  ON replica.id = witness.record_id
LEFT JOIN e2ee_replica_payload_hashes AS replica_hash
  ON replica_hash.record_id = replica.id
 AND replica_hash.workspace_id = replica.workspace_id
LEFT JOIN e2ee_ciphertext_archive AS archive
  ON archive.workspace_id = witness.workspace_id
 AND archive.record_id = witness.record_id
 AND archive.payload_hash = witness.payload_hash;

CREATE TRIGGER e2ee_records_archive_before_update
BEFORE UPDATE OF workspace_id, payload ON e2ee_records
WHEN OLD.payload <> ''
  AND (
    OLD.workspace_id <> NEW.workspace_id
    OR OLD.payload <> NEW.payload
  )
BEGIN
  INSERT OR IGNORE INTO e2ee_ciphertext_archive (
    workspace_id, record_id, payload_hash, payload
  )
  SELECT OLD.workspace_id, OLD.id, replica_hash.payload_hash, OLD.payload
  FROM e2ee_replica_payload_hashes AS replica_hash
  WHERE replica_hash.record_id = OLD.id
    AND replica_hash.workspace_id = OLD.workspace_id
    AND replica_hash.payload_hash <> ''
    AND NOT EXISTS (
      SELECT 1 FROM e2ee_ciphertext_archive AS archive
      WHERE archive.workspace_id = OLD.workspace_id
        AND archive.record_id = OLD.id
        AND archive.payload_hash = replica_hash.payload_hash
    )
    AND (
      EXISTS(
        SELECT 1 FROM e2ee_local_state AS local
        WHERE local.workspace_id = OLD.workspace_id
          AND local.record_id = OLD.id
          AND local.payload_hash = replica_hash.payload_hash
      )
      OR EXISTS(
        SELECT 1 FROM e2ee_witness_records AS witness
        WHERE witness.workspace_id = OLD.workspace_id
          AND witness.record_id = OLD.id
          AND witness.payload_hash = replica_hash.payload_hash
      )
    );
END;

CREATE TRIGGER e2ee_records_archive_before_delete
BEFORE DELETE ON e2ee_records
WHEN OLD.payload <> ''
BEGIN
  INSERT OR IGNORE INTO e2ee_ciphertext_archive (
    workspace_id, record_id, payload_hash, payload
  )
  SELECT OLD.workspace_id, OLD.id, replica_hash.payload_hash, OLD.payload
  FROM e2ee_replica_payload_hashes AS replica_hash
  WHERE replica_hash.record_id = OLD.id
    AND replica_hash.workspace_id = OLD.workspace_id
    AND replica_hash.payload_hash <> ''
    AND NOT EXISTS (
      SELECT 1 FROM e2ee_ciphertext_archive AS archive
      WHERE archive.workspace_id = OLD.workspace_id
        AND archive.record_id = OLD.id
        AND archive.payload_hash = replica_hash.payload_hash
    )
    AND (
      EXISTS(
        SELECT 1 FROM e2ee_local_state AS local
        WHERE local.workspace_id = OLD.workspace_id
          AND local.record_id = OLD.id
          AND local.payload_hash = replica_hash.payload_hash
      )
      OR EXISTS(
        SELECT 1 FROM e2ee_witness_records AS witness
        WHERE witness.workspace_id = OLD.workspace_id
          AND witness.record_id = OLD.id
          AND witness.payload_hash = replica_hash.payload_hash
      )
    );
END;

CREATE TRIGGER e2ee_replica_payload_hash_insert
AFTER INSERT ON e2ee_records
BEGIN
  INSERT INTO e2ee_replica_payload_hashes (record_id, workspace_id, payload_hash)
  VALUES (NEW.id, NEW.workspace_id, '')
  ON CONFLICT(record_id) DO UPDATE SET
    workspace_id = excluded.workspace_id,
    payload_hash = '',
    updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now');
END;

CREATE TRIGGER e2ee_replica_payload_hash_update
AFTER UPDATE OF workspace_id, payload ON e2ee_records
WHEN OLD.workspace_id <> NEW.workspace_id OR OLD.payload <> NEW.payload
BEGIN
  INSERT INTO e2ee_replica_payload_hashes (record_id, workspace_id, payload_hash)
  VALUES (NEW.id, NEW.workspace_id, '')
  ON CONFLICT(record_id) DO UPDATE SET
    workspace_id = excluded.workspace_id,
    payload_hash = '',
    updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now');
END;

CREATE TRIGGER e2ee_replica_payload_hash_delete
AFTER DELETE ON e2ee_records
BEGIN
  DELETE FROM e2ee_replica_payload_hashes WHERE record_id = OLD.id;
END;

CREATE TRIGGER e2ee_local_state_normalize_payload_insert
AFTER INSERT ON e2ee_local_state
WHEN NEW.payload <> ''
BEGIN
  INSERT OR IGNORE INTO e2ee_ciphertext_archive (
    workspace_id, record_id, payload_hash, payload
  )
  SELECT NEW.workspace_id, NEW.record_id, NEW.payload_hash, NEW.payload
  WHERE NOT EXISTS (
    SELECT 1
    FROM e2ee_records AS replica
    JOIN e2ee_replica_payload_hashes AS replica_hash
      ON replica_hash.record_id = replica.id
     AND replica_hash.workspace_id = replica.workspace_id
    WHERE replica.id = NEW.record_id
      AND replica.workspace_id = NEW.workspace_id
      AND replica.payload = NEW.payload
      AND replica_hash.payload_hash = NEW.payload_hash
  )
  AND NOT EXISTS (
    SELECT 1 FROM e2ee_ciphertext_archive AS archive
    WHERE archive.workspace_id = NEW.workspace_id
      AND archive.record_id = NEW.record_id
      AND archive.payload_hash = NEW.payload_hash
  );
  UPDATE e2ee_local_state SET payload = '' WHERE record_id = NEW.record_id;
END;

CREATE TRIGGER e2ee_local_state_normalize_payload_update
AFTER UPDATE OF payload ON e2ee_local_state
WHEN NEW.payload <> ''
BEGIN
  INSERT OR IGNORE INTO e2ee_ciphertext_archive (
    workspace_id, record_id, payload_hash, payload
  )
  SELECT NEW.workspace_id, NEW.record_id, NEW.payload_hash, NEW.payload
  WHERE NOT EXISTS (
    SELECT 1
    FROM e2ee_records AS replica
    JOIN e2ee_replica_payload_hashes AS replica_hash
      ON replica_hash.record_id = replica.id
     AND replica_hash.workspace_id = replica.workspace_id
    WHERE replica.id = NEW.record_id
      AND replica.workspace_id = NEW.workspace_id
      AND replica.payload = NEW.payload
      AND replica_hash.payload_hash = NEW.payload_hash
  )
  AND NOT EXISTS (
    SELECT 1 FROM e2ee_ciphertext_archive AS archive
    WHERE archive.workspace_id = NEW.workspace_id
      AND archive.record_id = NEW.record_id
      AND archive.payload_hash = NEW.payload_hash
  );
  UPDATE e2ee_local_state SET payload = '' WHERE record_id = NEW.record_id;
END;

CREATE TRIGGER e2ee_witness_records_normalize_payload_insert
AFTER INSERT ON e2ee_witness_records
WHEN NEW.payload <> ''
BEGIN
  INSERT OR IGNORE INTO e2ee_ciphertext_archive (
    workspace_id, record_id, payload_hash, payload
  )
  SELECT NEW.workspace_id, NEW.record_id, NEW.payload_hash, NEW.payload
  WHERE NOT EXISTS (
    SELECT 1
    FROM e2ee_records AS replica
    JOIN e2ee_replica_payload_hashes AS replica_hash
      ON replica_hash.record_id = replica.id
     AND replica_hash.workspace_id = replica.workspace_id
    WHERE replica.id = NEW.record_id
      AND replica.workspace_id = NEW.workspace_id
      AND replica.payload = NEW.payload
      AND replica_hash.payload_hash = NEW.payload_hash
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

CREATE TRIGGER e2ee_witness_records_normalize_payload_update
AFTER UPDATE OF payload ON e2ee_witness_records
WHEN NEW.payload <> ''
BEGIN
  INSERT OR IGNORE INTO e2ee_ciphertext_archive (
    workspace_id, record_id, payload_hash, payload
  )
  SELECT NEW.workspace_id, NEW.record_id, NEW.payload_hash, NEW.payload
  WHERE NOT EXISTS (
    SELECT 1
    FROM e2ee_records AS replica
    JOIN e2ee_replica_payload_hashes AS replica_hash
      ON replica_hash.record_id = replica.id
     AND replica_hash.workspace_id = replica.workspace_id
    WHERE replica.id = NEW.record_id
      AND replica.workspace_id = NEW.workspace_id
      AND replica.payload = NEW.payload
      AND replica_hash.payload_hash = NEW.payload_hash
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

CREATE TRIGGER e2ee_local_state_prune_archive_delete
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

CREATE TRIGGER e2ee_local_state_prune_archive_update
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

CREATE TRIGGER e2ee_witness_records_prune_archive_delete
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

CREATE TRIGGER e2ee_witness_records_prune_archive_update
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
