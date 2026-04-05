CREATE TABLE IF NOT EXISTS tasks (
  id             TEXT PRIMARY KEY NOT NULL,
  daily_id       TEXT NOT NULL DEFAULT '' REFERENCES daily(id),
  parent_task_id TEXT DEFAULT NULL REFERENCES tasks(id),
  sort_key       TEXT NOT NULL DEFAULT '',
  type           TEXT NOT NULL DEFAULT 'todo',
  title          TEXT NOT NULL DEFAULT '',
  status         TEXT NOT NULL DEFAULT 'open',
  source_id      TEXT NOT NULL DEFAULT '',
  source_url     TEXT NOT NULL DEFAULT '',
  event_id       TEXT DEFAULT NULL REFERENCES events(id),
  metadata_json  TEXT NOT NULL DEFAULT '{}',
  user_id        TEXT NOT NULL DEFAULT '',
  visibility     TEXT NOT NULL DEFAULT 'public',
  created_at     TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
  updated_at     TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
  updated_by     TEXT NOT NULL DEFAULT ''
);
CREATE INDEX IF NOT EXISTS idx_tasks_daily ON tasks(daily_id);
CREATE INDEX IF NOT EXISTS idx_tasks_parent ON tasks(parent_task_id);
CREATE INDEX IF NOT EXISTS idx_tasks_status ON tasks(status);
CREATE INDEX IF NOT EXISTS idx_tasks_type ON tasks(type);
CREATE INDEX IF NOT EXISTS idx_tasks_daily_sort ON tasks(daily_id, sort_key);
CREATE INDEX IF NOT EXISTS idx_tasks_event_id ON tasks(event_id);
CREATE UNIQUE INDEX IF NOT EXISTS idx_tasks_event_id_unique ON tasks(event_id) WHERE event_id IS NOT NULL;
