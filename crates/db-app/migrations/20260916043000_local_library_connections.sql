-- Local routing only. Encryption keys, replica records, and cursors continue
-- to use the remote workspace identity.
CREATE TABLE local_library_connections (
  account_user_id TEXT PRIMARY KEY NOT NULL,
  library_workspace_id TEXT NOT NULL DEFAULT '',
  active INTEGER NOT NULL DEFAULT 0,
  recovery_settings TEXT NOT NULL DEFAULT '{}',
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
) STRICT;

CREATE INDEX idx_local_library_connections_library
ON local_library_connections(library_workspace_id);

CREATE UNIQUE INDEX idx_local_library_connections_active
ON local_library_connections(active) WHERE active = 1;

CREATE TABLE local_library_attachment_state (
  account_user_id TEXT NOT NULL,
  attachment_id TEXT NOT NULL,
  cloud_object_key TEXT NOT NULL DEFAULT '',
  cloud_sync_enabled INTEGER NOT NULL DEFAULT 0,
  PRIMARY KEY (account_user_id, attachment_id)
) STRICT;

CREATE TRIGGER local_library_attachment_insert
AFTER INSERT ON session_attachments
BEGIN
  INSERT INTO local_library_attachment_state
    (account_user_id, attachment_id, cloud_object_key, cloud_sync_enabled)
  SELECT account_user_id, NEW.id, NEW.cloud_object_key, NEW.cloud_sync_enabled
  FROM local_library_connections WHERE active = 1 AND library_workspace_id = NEW.workspace_id
  ON CONFLICT(account_user_id, attachment_id) DO UPDATE SET
    cloud_object_key = excluded.cloud_object_key,
    cloud_sync_enabled = excluded.cloud_sync_enabled;
END;

CREATE TRIGGER local_library_attachment_update
AFTER UPDATE OF cloud_object_key, cloud_sync_enabled ON session_attachments
BEGIN
  INSERT INTO local_library_attachment_state
    (account_user_id, attachment_id, cloud_object_key, cloud_sync_enabled)
  SELECT account_user_id, NEW.id, NEW.cloud_object_key, NEW.cloud_sync_enabled
  FROM local_library_connections WHERE active = 1 AND library_workspace_id = NEW.workspace_id
  ON CONFLICT(account_user_id, attachment_id) DO UPDATE SET
    cloud_object_key = excluded.cloud_object_key,
    cloud_sync_enabled = excluded.cloud_sync_enabled;
END;

CREATE TRIGGER local_library_dirty_insert
AFTER INSERT ON e2ee_dirty_rows
WHEN EXISTS (SELECT 1 FROM local_library_connections WHERE library_workspace_id = NEW.workspace_id)
  AND NOT EXISTS (
    SELECT 1 FROM e2ee_apply_guard
    WHERE workspace_id = NEW.workspace_id AND table_name = NEW.table_name AND row_id = NEW.row_id
  )
BEGIN
  INSERT INTO e2ee_dirty_rows (workspace_id, table_name, row_id, dirtied_at_ms)
  SELECT account_user_id, NEW.table_name,
    CASE WHEN NEW.table_name = 'humans' AND NEW.row_id = library_workspace_id
      THEN account_user_id ELSE NEW.row_id END,
    NEW.dirtied_at_ms
  FROM local_library_connections
  WHERE library_workspace_id = NEW.workspace_id
    AND account_user_id != NEW.workspace_id
  ON CONFLICT(workspace_id, table_name, row_id) DO UPDATE SET
    generation = e2ee_dirty_rows.generation + 1,
    dirtied_at_ms = MAX(e2ee_dirty_rows.dirtied_at_ms, excluded.dirtied_at_ms);
END;

CREATE TRIGGER local_library_dirty_update
AFTER UPDATE OF generation ON e2ee_dirty_rows
WHEN NEW.generation > OLD.generation
  AND EXISTS (SELECT 1 FROM local_library_connections WHERE library_workspace_id = NEW.workspace_id)
  AND NOT EXISTS (
    SELECT 1 FROM e2ee_apply_guard
    WHERE workspace_id = NEW.workspace_id AND table_name = NEW.table_name AND row_id = NEW.row_id
  )
BEGIN
  INSERT INTO e2ee_dirty_rows (workspace_id, table_name, row_id, dirtied_at_ms)
  SELECT account_user_id, NEW.table_name,
    CASE WHEN NEW.table_name = 'humans' AND NEW.row_id = library_workspace_id
      THEN account_user_id ELSE NEW.row_id END,
    NEW.dirtied_at_ms
  FROM local_library_connections
  WHERE library_workspace_id = NEW.workspace_id
    AND account_user_id != NEW.workspace_id
  ON CONFLICT(workspace_id, table_name, row_id) DO UPDATE SET
    generation = e2ee_dirty_rows.generation + 1,
    dirtied_at_ms = MAX(e2ee_dirty_rows.dirtied_at_ms, excluded.dirtied_at_ms);
END;


-- Incoming edits retain their original write time when queued for another
-- connection. A local edit still receives the time at which it was written.
DROP TRIGGER e2ee_write_time_stamp_insert;
CREATE TRIGGER e2ee_write_time_stamp_insert
AFTER INSERT ON e2ee_dirty_rows
WHEN NEW.dirtied_at_ms = 0
BEGIN
  UPDATE e2ee_dirty_rows
  SET dirtied_at_ms = CAST((julianday('now') - 2440587.5) * 86400000 AS INTEGER)
  WHERE workspace_id = NEW.workspace_id AND table_name = NEW.table_name AND row_id = NEW.row_id;
END;

DROP TRIGGER e2ee_write_time_stamp_update;
CREATE TRIGGER e2ee_write_time_stamp_update
AFTER UPDATE OF generation ON e2ee_dirty_rows
WHEN NEW.generation > OLD.generation AND NOT EXISTS (
  SELECT 1 FROM e2ee_apply_guard AS guard
  JOIN local_library_connections AS connection ON connection.account_user_id = NEW.workspace_id AND connection.active = 0
  WHERE guard.workspace_id = COALESCE(connection.library_workspace_id, NEW.workspace_id)
    AND guard.table_name = NEW.table_name
    AND guard.row_id = CASE WHEN NEW.table_name = 'humans' AND NEW.row_id = NEW.workspace_id
      THEN COALESCE(connection.library_workspace_id, NEW.row_id) ELSE NEW.row_id END
)
BEGIN
  UPDATE e2ee_dirty_rows
  SET dirtied_at_ms = CAST((julianday('now') - 2440587.5) * 86400000 AS INTEGER)
  WHERE workspace_id = NEW.workspace_id AND table_name = NEW.table_name AND row_id = NEW.row_id;
END;
