CREATE TABLE IF NOT EXISTS meeting_artifacts (
  id            TEXT PRIMARY KEY NOT NULL,
  task_id       TEXT NOT NULL DEFAULT '' REFERENCES tasks(id),
  transcript_md TEXT NOT NULL DEFAULT '',
  note_body     TEXT NOT NULL DEFAULT '{}',
  user_id       TEXT NOT NULL DEFAULT '',
  visibility    TEXT NOT NULL DEFAULT 'public',
  created_at    TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
  updated_at    TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
);
CREATE UNIQUE INDEX IF NOT EXISTS idx_meeting_artifacts_task ON meeting_artifacts(task_id);

CREATE TABLE IF NOT EXISTS meeting_summaries (
  id          TEXT PRIMARY KEY NOT NULL,
  task_id     TEXT NOT NULL DEFAULT '' REFERENCES tasks(id),
  template_id TEXT NOT NULL DEFAULT '',
  content     TEXT NOT NULL DEFAULT '{}',
  position    INTEGER NOT NULL DEFAULT 0,
  title       TEXT NOT NULL DEFAULT '',
  user_id     TEXT NOT NULL DEFAULT '',
  visibility  TEXT NOT NULL DEFAULT 'public',
  created_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
  updated_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
);
CREATE INDEX IF NOT EXISTS idx_meeting_summaries_task ON meeting_summaries(task_id);
