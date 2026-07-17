CREATE TRIGGER IF NOT EXISTS search_index_humans_insert
AFTER INSERT ON humans
BEGIN
  INSERT INTO search_index_dirty (entity_type, entity_id)
  VALUES ('human', NEW.id)
  ON CONFLICT (entity_type, entity_id) DO UPDATE SET
    generation = search_index_dirty.generation + 1,
    queued_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now');
END;

CREATE TRIGGER IF NOT EXISTS search_index_humans_update
AFTER UPDATE ON humans
BEGIN
  INSERT INTO search_index_dirty (entity_type, entity_id)
  VALUES ('human', OLD.id)
  ON CONFLICT (entity_type, entity_id) DO UPDATE SET
    generation = search_index_dirty.generation + 1,
    queued_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now');

  INSERT INTO search_index_dirty (entity_type, entity_id)
  SELECT 'human', NEW.id
  WHERE NEW.id <> OLD.id
  ON CONFLICT (entity_type, entity_id) DO UPDATE SET
    generation = search_index_dirty.generation + 1,
    queued_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now');
END;

CREATE TRIGGER IF NOT EXISTS search_index_humans_delete
AFTER DELETE ON humans
BEGIN
  INSERT INTO search_index_dirty (entity_type, entity_id)
  VALUES ('human', OLD.id)
  ON CONFLICT (entity_type, entity_id) DO UPDATE SET
    generation = search_index_dirty.generation + 1,
    queued_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now');
END;
