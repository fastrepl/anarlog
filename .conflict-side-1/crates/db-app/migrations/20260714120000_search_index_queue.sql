CREATE TABLE IF NOT EXISTS search_index_dirty (
  entity_type  TEXT NOT NULL,
  entity_id    TEXT NOT NULL,
  generation   INTEGER NOT NULL DEFAULT 1,
  queued_at    TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  PRIMARY KEY (entity_type, entity_id)
) STRICT;

CREATE TABLE IF NOT EXISTS search_index_state (
  id                  TEXT PRIMARY KEY NOT NULL DEFAULT 'default' CHECK (id = 'default'),
  projection_version  INTEGER NOT NULL DEFAULT 0,
  updated_at          TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
) STRICT;

INSERT OR IGNORE INTO search_index_state (id, projection_version)
VALUES ('default', 0);
