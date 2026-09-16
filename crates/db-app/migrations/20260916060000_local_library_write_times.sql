-- Forwarded dirty rows already carry the local edit time. Only local writes
-- receive a new timestamp; queued copies must preserve conflict ordering.
DROP TRIGGER e2ee_write_time_stamp_update;
CREATE TRIGGER e2ee_write_time_stamp_update
AFTER UPDATE OF generation ON e2ee_dirty_rows
WHEN NEW.generation > OLD.generation
  AND NOT EXISTS (
    SELECT 1 FROM local_library_connections
    WHERE account_user_id = NEW.workspace_id AND library_workspace_id != NEW.workspace_id
  )
  AND NOT EXISTS (
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

-- Local generation updates can run before the local timestamp trigger. Both
-- use SQLite's statement-stable time so forwarded copies receive the same edit time.
DROP TRIGGER local_library_dirty_update;
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
    CAST((julianday('now') - 2440587.5) * 86400000 AS INTEGER)
  FROM local_library_connections
  WHERE library_workspace_id = NEW.workspace_id
    AND account_user_id != NEW.workspace_id
  ON CONFLICT(workspace_id, table_name, row_id) DO UPDATE SET
    generation = e2ee_dirty_rows.generation + 1,
    dirtied_at_ms = MAX(e2ee_dirty_rows.dirtied_at_ms, excluded.dirtied_at_ms);
END;
