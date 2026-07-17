ALTER TABLE migration_import_runs
ADD COLUMN matched_count INTEGER NOT NULL DEFAULT 0;

ALTER TABLE migration_import_items
ADD COLUMN matched_count INTEGER NOT NULL DEFAULT 0;

CREATE TABLE migration_import_targets (
  id           TEXT PRIMARY KEY NOT NULL,
  run_id       TEXT NOT NULL DEFAULT '',
  item_id      TEXT NOT NULL DEFAULT '',
  source_path  TEXT NOT NULL DEFAULT '',
  source_kind  TEXT NOT NULL DEFAULT '',
  table_name   TEXT NOT NULL DEFAULT '',
  target_id    TEXT NOT NULL DEFAULT '',
  status       TEXT NOT NULL DEFAULT 'pending',
  error        TEXT NOT NULL DEFAULT '',
  created_at   TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
) STRICT;

CREATE INDEX idx_migration_import_targets_run_id
ON migration_import_targets(run_id);

CREATE INDEX idx_migration_import_targets_table
ON migration_import_targets(run_id, table_name, status);

CREATE INDEX idx_migration_import_targets_source
ON migration_import_targets(source_path, target_id);
