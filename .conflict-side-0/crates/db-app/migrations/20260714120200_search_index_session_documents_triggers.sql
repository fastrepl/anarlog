CREATE TRIGGER IF NOT EXISTS search_index_session_documents_insert
AFTER INSERT ON session_documents
WHEN NEW.session_id <> ''
BEGIN
  INSERT INTO search_index_dirty (entity_type, entity_id)
  VALUES ('session', NEW.session_id)
  ON CONFLICT (entity_type, entity_id) DO UPDATE SET
    generation = search_index_dirty.generation + 1,
    queued_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now');
END;

CREATE TRIGGER IF NOT EXISTS search_index_session_documents_update
AFTER UPDATE ON session_documents
BEGIN
  INSERT INTO search_index_dirty (entity_type, entity_id)
  SELECT 'session', OLD.session_id
  WHERE OLD.session_id <> ''
  ON CONFLICT (entity_type, entity_id) DO UPDATE SET
    generation = search_index_dirty.generation + 1,
    queued_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now');

  INSERT INTO search_index_dirty (entity_type, entity_id)
  SELECT 'session', NEW.session_id
  WHERE NEW.session_id <> '' AND NEW.session_id <> OLD.session_id
  ON CONFLICT (entity_type, entity_id) DO UPDATE SET
    generation = search_index_dirty.generation + 1,
    queued_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now');
END;

CREATE TRIGGER IF NOT EXISTS search_index_session_documents_delete
AFTER DELETE ON session_documents
WHEN OLD.session_id <> ''
BEGIN
  INSERT INTO search_index_dirty (entity_type, entity_id)
  VALUES ('session', OLD.session_id)
  ON CONFLICT (entity_type, entity_id) DO UPDATE SET
    generation = search_index_dirty.generation + 1,
    queued_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now');
END;
