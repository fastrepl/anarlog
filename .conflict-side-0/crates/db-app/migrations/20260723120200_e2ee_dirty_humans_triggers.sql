CREATE TRIGGER IF NOT EXISTS e2ee_dirty_humans_insert
AFTER INSERT ON humans
WHEN NOT EXISTS (
  SELECT 1
  FROM e2ee_apply_guard
  WHERE workspace_id = NEW.workspace_id
    AND table_name = 'humans'
    AND row_id = NEW.id
)
BEGIN
  INSERT INTO e2ee_dirty_rows (workspace_id, table_name, row_id)
  VALUES (NEW.workspace_id, 'humans', NEW.id)
  ON CONFLICT (workspace_id, table_name, row_id) DO UPDATE SET
    generation = e2ee_dirty_rows.generation + 1;
END;

CREATE TRIGGER IF NOT EXISTS e2ee_dirty_humans_update
AFTER UPDATE ON humans
BEGIN
  INSERT INTO e2ee_dirty_rows (workspace_id, table_name, row_id)
  SELECT OLD.workspace_id, 'humans', OLD.id
  WHERE NOT EXISTS (
    SELECT 1
    FROM e2ee_apply_guard
    WHERE workspace_id = OLD.workspace_id
      AND table_name = 'humans'
      AND row_id = OLD.id
  )
  ON CONFLICT (workspace_id, table_name, row_id) DO UPDATE SET
    generation = e2ee_dirty_rows.generation + 1;

  INSERT INTO e2ee_dirty_rows (workspace_id, table_name, row_id)
  SELECT NEW.workspace_id, 'humans', NEW.id
  WHERE (NEW.workspace_id <> OLD.workspace_id OR NEW.id <> OLD.id)
    AND NOT EXISTS (
    SELECT 1
    FROM e2ee_apply_guard
    WHERE workspace_id = NEW.workspace_id
      AND table_name = 'humans'
      AND row_id = NEW.id
  )
  ON CONFLICT (workspace_id, table_name, row_id) DO UPDATE SET
    generation = e2ee_dirty_rows.generation + 1;
END;

CREATE TRIGGER IF NOT EXISTS e2ee_dirty_humans_delete
AFTER DELETE ON humans
WHEN NOT EXISTS (
  SELECT 1
  FROM e2ee_apply_guard
  WHERE workspace_id = OLD.workspace_id
    AND table_name = 'humans'
    AND row_id = OLD.id
)
BEGIN
  INSERT INTO e2ee_dirty_rows (workspace_id, table_name, row_id)
  VALUES (OLD.workspace_id, 'humans', OLD.id)
  ON CONFLICT (workspace_id, table_name, row_id) DO UPDATE SET
    generation = e2ee_dirty_rows.generation + 1;
END;
