-- Meeting import history: a finished Fathom sync, shown under its row once connected.
INSERT INTO
  migration_import_runs (id, importer_version, source_root, dry_run, status, discovered_count, imported_count, matched_count, started_at, completed_at)
VALUES
  ('run-fathom', 2, 'meeting-import:fathom', 0, 'completed', 4, 3, 1, '2026-11-10T09:00:00.000Z', '2026-11-10T09:00:05.000Z');
