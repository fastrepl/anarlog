CREATE TABLE IF NOT EXISTS tasks (
  id            TEXT PRIMARY KEY NOT NULL,
  source_type   TEXT NOT NULL DEFAULT '',
  source_id     TEXT NOT NULL DEFAULT '',
  source_order  INTEGER NOT NULL DEFAULT 0,
  status        TEXT NOT NULL DEFAULT 'todo',
  text_preview  TEXT NOT NULL DEFAULT '',
  body_json     TEXT NOT NULL DEFAULT '[]',
  due_date      TEXT,
  created_at    TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
  updated_at    TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
  CHECK (status IN ('todo', 'in_progress', 'done'))
);

CREATE INDEX IF NOT EXISTS idx_tasks_source_order
  ON tasks(source_type, source_id, source_order);

CREATE INDEX IF NOT EXISTS idx_tasks_status
  ON tasks(status);
