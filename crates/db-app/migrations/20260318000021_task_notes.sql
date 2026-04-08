CREATE TABLE IF NOT EXISTS task_notes (
  id          TEXT PRIMARY KEY NOT NULL,
  task_id     TEXT NOT NULL DEFAULT '' REFERENCES tasks(id),
  author_type TEXT NOT NULL DEFAULT '',
  author_id   TEXT NOT NULL DEFAULT '',
  body        TEXT NOT NULL DEFAULT '',
  user_id     TEXT NOT NULL DEFAULT '',
  visibility  TEXT NOT NULL DEFAULT 'public',
  created_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
  deleted_at  TEXT DEFAULT NULL
);
CREATE INDEX IF NOT EXISTS idx_task_notes_task ON task_notes(task_id);
