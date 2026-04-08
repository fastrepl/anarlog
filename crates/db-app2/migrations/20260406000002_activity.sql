CREATE TABLE IF NOT EXISTS activity_signals (
  id                   TEXT PRIMARY KEY NOT NULL,
  occurred_at_ms       INTEGER NOT NULL DEFAULT 0,
  transition_sequence  INTEGER NOT NULL DEFAULT 0,
  reason               TEXT NOT NULL DEFAULT '',
  app_id               TEXT NOT NULL DEFAULT '',
  bundle_id            TEXT NOT NULL DEFAULT '',
  app_name             TEXT NOT NULL DEFAULT '',
  activity_kind        TEXT NOT NULL DEFAULT '',
  window_title         TEXT NOT NULL DEFAULT '',
  url                  TEXT NOT NULL DEFAULT '',
  domain               TEXT NOT NULL DEFAULT '',
  content_level        TEXT NOT NULL DEFAULT '',
  source               TEXT NOT NULL DEFAULT '',
  text_anchor_identity TEXT NOT NULL DEFAULT '',
  fingerprint          TEXT NOT NULL DEFAULT '',
  payload_json         TEXT NOT NULL DEFAULT '{}',
  created_at           TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
);

CREATE INDEX IF NOT EXISTS idx_activity_signals_occurred_at_ms ON activity_signals(occurred_at_ms);
CREATE INDEX IF NOT EXISTS idx_activity_signals_sequence ON activity_signals(transition_sequence);
CREATE INDEX IF NOT EXISTS idx_activity_signals_domain ON activity_signals(domain);
CREATE INDEX IF NOT EXISTS idx_activity_signals_app_id ON activity_signals(app_id);

CREATE TABLE IF NOT EXISTS activity_segments (
  id                     TEXT PRIMARY KEY NOT NULL,
  started_at_ms          INTEGER NOT NULL DEFAULT 0,
  ended_at_ms            INTEGER NOT NULL DEFAULT 0,
  duration_ms            INTEGER NOT NULL DEFAULT 0,
  date                   TEXT NOT NULL DEFAULT '',
  semantic_key           TEXT NOT NULL DEFAULT '',
  app_id                 TEXT NOT NULL DEFAULT '',
  bundle_id              TEXT NOT NULL DEFAULT '',
  app_name               TEXT NOT NULL DEFAULT '',
  activity_kind          TEXT NOT NULL DEFAULT '',
  title                  TEXT NOT NULL DEFAULT '',
  url                    TEXT NOT NULL DEFAULT '',
  domain                 TEXT NOT NULL DEFAULT '',
  payload_json           TEXT NOT NULL DEFAULT '{}',
  source_signal_start_id TEXT DEFAULT NULL REFERENCES activity_signals(id) ON DELETE SET NULL,
  source_signal_end_id   TEXT DEFAULT NULL REFERENCES activity_signals(id) ON DELETE SET NULL,
  linked_task_id         TEXT DEFAULT NULL REFERENCES tasks(id) ON DELETE SET NULL,
  linked_daily_note_id   TEXT DEFAULT NULL REFERENCES daily_notes(id) ON DELETE SET NULL,
  created_at             TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
  updated_at             TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
);

CREATE INDEX IF NOT EXISTS idx_activity_segments_date ON activity_segments(date);
CREATE INDEX IF NOT EXISTS idx_activity_segments_time_range ON activity_segments(started_at_ms, ended_at_ms);
CREATE INDEX IF NOT EXISTS idx_activity_segments_semantic_key ON activity_segments(semantic_key);
CREATE INDEX IF NOT EXISTS idx_activity_segments_linked_task ON activity_segments(linked_task_id);
