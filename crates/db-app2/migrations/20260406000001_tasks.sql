CREATE TABLE IF NOT EXISTS tasks (
  id             TEXT PRIMARY KEY NOT NULL,
  daily_note_id  TEXT NOT NULL DEFAULT '' REFERENCES daily_notes(id) ON DELETE CASCADE,
  parent_task_id TEXT DEFAULT NULL REFERENCES tasks(id) ON DELETE SET NULL,
  sort_key       TEXT NOT NULL DEFAULT '',
  kind           TEXT NOT NULL DEFAULT 'todo',
  title          TEXT NOT NULL DEFAULT '',
  status         TEXT NOT NULL DEFAULT 'todo',
  body_json      TEXT NOT NULL DEFAULT '{}',
  source_type    TEXT NOT NULL DEFAULT '',
  source_id      TEXT NOT NULL DEFAULT '',
  due_date       TEXT DEFAULT NULL,
  metadata_json  TEXT NOT NULL DEFAULT '{}',
  user_id        TEXT NOT NULL DEFAULT '',
  created_at     TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
  updated_at     TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
);

CREATE INDEX IF NOT EXISTS idx_tasks_daily_note ON tasks(daily_note_id);
CREATE INDEX IF NOT EXISTS idx_tasks_parent ON tasks(parent_task_id);
CREATE INDEX IF NOT EXISTS idx_tasks_status ON tasks(status);
CREATE INDEX IF NOT EXISTS idx_tasks_kind ON tasks(kind);
CREATE INDEX IF NOT EXISTS idx_tasks_daily_note_sort ON tasks(daily_note_id, sort_key);
CREATE INDEX IF NOT EXISTS idx_tasks_source ON tasks(source_type, source_id);
