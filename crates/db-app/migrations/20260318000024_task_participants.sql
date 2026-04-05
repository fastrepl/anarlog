CREATE TABLE IF NOT EXISTS task_participants (
  id       TEXT PRIMARY KEY NOT NULL,
  task_id  TEXT NOT NULL DEFAULT '' REFERENCES tasks(id),
  human_id TEXT NOT NULL DEFAULT '' REFERENCES humans(id),
  source     TEXT NOT NULL DEFAULT 'manual',
  user_id    TEXT NOT NULL DEFAULT '',
  visibility TEXT NOT NULL DEFAULT 'public'
);
CREATE INDEX IF NOT EXISTS idx_tp_task ON task_participants(task_id);
CREATE INDEX IF NOT EXISTS idx_tp_human ON task_participants(human_id);
CREATE UNIQUE INDEX IF NOT EXISTS idx_tp_task_human ON task_participants(task_id, human_id);
