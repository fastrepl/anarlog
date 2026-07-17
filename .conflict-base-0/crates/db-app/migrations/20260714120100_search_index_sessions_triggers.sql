CREATE TRIGGER IF NOT EXISTS search_index_sessions_insert
AFTER INSERT ON sessions
BEGIN
  INSERT INTO search_index_dirty (entity_type, entity_id)
  VALUES ('session', NEW.id)
  ON CONFLICT (entity_type, entity_id) DO UPDATE SET
    generation = search_index_dirty.generation + 1,
    queued_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now');
END;

CREATE TRIGGER IF NOT EXISTS search_index_sessions_update
AFTER UPDATE ON sessions
BEGIN
  INSERT INTO search_index_dirty (entity_type, entity_id)
  VALUES ('session', OLD.id)
  ON CONFLICT (entity_type, entity_id) DO UPDATE SET
    generation = search_index_dirty.generation + 1,
    queued_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now');

  INSERT INTO search_index_dirty (entity_type, entity_id)
  SELECT 'session', NEW.id
  WHERE NEW.id <> OLD.id
  ON CONFLICT (entity_type, entity_id) DO UPDATE SET
    generation = search_index_dirty.generation + 1,
    queued_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now');
END;

CREATE TRIGGER IF NOT EXISTS search_index_sessions_delete
AFTER DELETE ON sessions
BEGIN
  INSERT INTO search_index_dirty (entity_type, entity_id)
  VALUES ('session', OLD.id)
  ON CONFLICT (entity_type, entity_id) DO UPDATE SET
    generation = search_index_dirty.generation + 1,
    queued_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now');
END;
