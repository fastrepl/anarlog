CREATE TABLE IF NOT EXISTS task_events (
  id           TEXT PRIMARY KEY NOT NULL,
  task_id      TEXT NOT NULL DEFAULT '' REFERENCES tasks(id),
  actor_type   TEXT NOT NULL DEFAULT '',
  actor_id     TEXT NOT NULL DEFAULT '',
  event_type   TEXT NOT NULL DEFAULT '',
  payload_json TEXT NOT NULL DEFAULT '{}',
  user_id      TEXT NOT NULL DEFAULT '',
  visibility   TEXT NOT NULL DEFAULT 'public',
  created_at   TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
);
CREATE INDEX IF NOT EXISTS idx_task_events_task ON task_events(task_id);
CREATE INDEX IF NOT EXISTS idx_task_events_type ON task_events(event_type);
