CREATE TABLE IF NOT EXISTS slack_teams (
  id            TEXT PRIMARY KEY NOT NULL,
  connection_id TEXT NOT NULL DEFAULT '',
  team_id       TEXT NOT NULL DEFAULT '',
  team_name     TEXT NOT NULL DEFAULT '',
  created_at    TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
  user_id       TEXT NOT NULL DEFAULT ''
);
CREATE UNIQUE INDEX IF NOT EXISTS idx_slack_teams_team ON slack_teams(team_id);

CREATE TABLE IF NOT EXISTS slack_channels (
  id            TEXT PRIMARY KEY NOT NULL,
  slack_team_id TEXT NOT NULL DEFAULT '',
  channel_id    TEXT NOT NULL DEFAULT '',
  name          TEXT NOT NULL DEFAULT '',
  channel_type  TEXT NOT NULL DEFAULT '',
  is_external   INTEGER NOT NULL DEFAULT 0,
  created_at    TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
  user_id       TEXT NOT NULL DEFAULT ''
);
CREATE UNIQUE INDEX IF NOT EXISTS idx_slack_channels_lookup ON slack_channels(slack_team_id, channel_id);

CREATE TABLE IF NOT EXISTS slack_threads (
  id              TEXT PRIMARY KEY NOT NULL,
  channel_id      TEXT NOT NULL DEFAULT '',
  thread_ts       TEXT NOT NULL DEFAULT '',
  started_at      TEXT NOT NULL DEFAULT '',
  last_message_at TEXT NOT NULL DEFAULT '',
  message_count   INTEGER NOT NULL DEFAULT 0,
  created_at      TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
  user_id         TEXT NOT NULL DEFAULT ''
);
CREATE INDEX IF NOT EXISTS idx_slack_threads_channel ON slack_threads(channel_id);
CREATE UNIQUE INDEX IF NOT EXISTS idx_slack_threads_lookup ON slack_threads(channel_id, thread_ts);

CREATE TABLE IF NOT EXISTS slack_messages (
  id         TEXT PRIMARY KEY NOT NULL,
  thread_id  TEXT NOT NULL DEFAULT '',
  channel_id TEXT NOT NULL DEFAULT '',
  alias_id   TEXT NOT NULL DEFAULT '',
  text       TEXT NOT NULL DEFAULT '',
  ts         TEXT NOT NULL DEFAULT '',
  raw_json   TEXT NOT NULL DEFAULT '{}',
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
  user_id    TEXT NOT NULL DEFAULT ''
);
CREATE INDEX IF NOT EXISTS idx_slack_messages_thread ON slack_messages(thread_id);
CREATE INDEX IF NOT EXISTS idx_slack_messages_alias ON slack_messages(alias_id);

CREATE TABLE IF NOT EXISTS slack_thread_participants (
  id         TEXT PRIMARY KEY NOT NULL,
  thread_id  TEXT NOT NULL DEFAULT '',
  alias_id   TEXT NOT NULL DEFAULT '',
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
  user_id    TEXT NOT NULL DEFAULT ''
);
CREATE UNIQUE INDEX IF NOT EXISTS idx_stp_lookup ON slack_thread_participants(thread_id, alias_id);
CREATE INDEX IF NOT EXISTS idx_stp_alias ON slack_thread_participants(alias_id);
