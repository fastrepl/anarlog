CREATE TABLE IF NOT EXISTS organizations (
  id             TEXT PRIMARY KEY NOT NULL,
  workspace_id   TEXT NOT NULL DEFAULT '',
  owner_user_id  TEXT NOT NULL DEFAULT '',
  name           TEXT NOT NULL DEFAULT '',
  memo           TEXT NOT NULL DEFAULT '',
  pinned         INTEGER NOT NULL DEFAULT 0,
  pin_order      INTEGER,
  metadata_json  TEXT NOT NULL DEFAULT '{}',
  created_at     TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  updated_at     TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  deleted_at     TEXT
) STRICT;

CREATE TABLE IF NOT EXISTS humans (
  id                 TEXT PRIMARY KEY NOT NULL,
  workspace_id       TEXT NOT NULL DEFAULT '',
  owner_user_id      TEXT NOT NULL DEFAULT '',
  organization_id    TEXT NOT NULL DEFAULT '',
  name               TEXT NOT NULL DEFAULT '',
  email              TEXT NOT NULL DEFAULT '',
  phone              TEXT NOT NULL DEFAULT '',
  job_title          TEXT NOT NULL DEFAULT '',
  linkedin_username  TEXT NOT NULL DEFAULT '',
  memo               TEXT NOT NULL DEFAULT '',
  pinned             INTEGER NOT NULL DEFAULT 0,
  pin_order          INTEGER,
  metadata_json      TEXT NOT NULL DEFAULT '{}',
  created_at         TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  updated_at         TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  deleted_at         TEXT
) STRICT;

CREATE TABLE IF NOT EXISTS sessions (
  id                   TEXT PRIMARY KEY NOT NULL,
  workspace_id         TEXT NOT NULL DEFAULT '',
  owner_user_id        TEXT NOT NULL DEFAULT '',
  title                TEXT NOT NULL DEFAULT '',
  kind                 TEXT NOT NULL DEFAULT 'meeting',
  status               TEXT NOT NULL DEFAULT 'active',
  created_at           TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  updated_at           TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  started_at           TEXT NOT NULL DEFAULT '',
  ended_at             TEXT NOT NULL DEFAULT '',
  timezone             TEXT NOT NULL DEFAULT '',
  language             TEXT NOT NULL DEFAULT '',
  event_id             TEXT NOT NULL DEFAULT '',
  external_event_id    TEXT NOT NULL DEFAULT '',
  external_provider    TEXT NOT NULL DEFAULT '',
  series_id            TEXT NOT NULL DEFAULT '',
  source_apps_json     TEXT NOT NULL DEFAULT '[]',
  event_json           TEXT NOT NULL DEFAULT '',
  folder_path          TEXT NOT NULL DEFAULT '',
  slug                 TEXT NOT NULL DEFAULT '',
  metadata_json        TEXT NOT NULL DEFAULT '{}',
  deleted_at           TEXT
) STRICT;

CREATE TABLE IF NOT EXISTS session_documents (
  id                        TEXT PRIMARY KEY NOT NULL,
  workspace_id              TEXT NOT NULL DEFAULT '',
  session_id                TEXT NOT NULL DEFAULT '',
  kind                      TEXT NOT NULL DEFAULT 'note',
  template_id               TEXT NOT NULL DEFAULT '',
  title                     TEXT NOT NULL DEFAULT '',
  body_format               TEXT NOT NULL DEFAULT 'prosemirror_json',
  body                      TEXT NOT NULL DEFAULT '',
  source_hash               TEXT NOT NULL DEFAULT '',
  generation_metadata_json  TEXT NOT NULL DEFAULT '{}',
  sort_order                INTEGER NOT NULL DEFAULT 0,
  created_by                TEXT NOT NULL DEFAULT '',
  updated_by                TEXT NOT NULL DEFAULT '',
  created_at                TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  updated_at                TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  deleted_at                TEXT
) STRICT;

CREATE TABLE IF NOT EXISTS transcripts (
  id                    TEXT PRIMARY KEY NOT NULL,
  workspace_id          TEXT NOT NULL DEFAULT '',
  owner_user_id         TEXT NOT NULL DEFAULT '',
  session_id            TEXT NOT NULL DEFAULT '',
  source                TEXT NOT NULL DEFAULT '',
  provider              TEXT NOT NULL DEFAULT '',
  model                 TEXT NOT NULL DEFAULT '',
  language              TEXT NOT NULL DEFAULT '',
  started_at_ms         INTEGER NOT NULL DEFAULT 0,
  ended_at_ms           INTEGER,
  audio_attachment_id   TEXT NOT NULL DEFAULT '',
  memo                   TEXT NOT NULL DEFAULT '',
  words_json            TEXT NOT NULL DEFAULT '[]',
  speaker_hints_json    TEXT NOT NULL DEFAULT '[]',
  metadata_json         TEXT NOT NULL DEFAULT '{}',
  created_at            TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  updated_at            TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  deleted_at            TEXT
) STRICT;

CREATE TABLE IF NOT EXISTS session_participants (
  id             TEXT PRIMARY KEY NOT NULL,
  workspace_id   TEXT NOT NULL DEFAULT '',
  owner_user_id  TEXT NOT NULL DEFAULT '',
  session_id     TEXT NOT NULL DEFAULT '',
  human_id       TEXT NOT NULL DEFAULT '',
  display_name   TEXT NOT NULL DEFAULT '',
  email          TEXT NOT NULL DEFAULT '',
  role           TEXT NOT NULL DEFAULT '',
  source         TEXT NOT NULL DEFAULT '',
  metadata_json  TEXT NOT NULL DEFAULT '{}',
  created_at     TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  updated_at     TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  deleted_at     TEXT
) STRICT;

CREATE TABLE IF NOT EXISTS action_items (
  id                 TEXT PRIMARY KEY NOT NULL,
  workspace_id       TEXT NOT NULL DEFAULT '',
  session_id         TEXT NOT NULL DEFAULT '',
  source_type        TEXT NOT NULL DEFAULT '',
  source_id          TEXT NOT NULL DEFAULT '',
  source_order       INTEGER NOT NULL DEFAULT 0,
  assignee_human_id  TEXT NOT NULL DEFAULT '',
  status             TEXT NOT NULL DEFAULT 'todo',
  text               TEXT NOT NULL DEFAULT '',
  body_json          TEXT NOT NULL DEFAULT '{}',
  due_at             TEXT NOT NULL DEFAULT '',
  completed_at       TEXT,
  created_by         TEXT NOT NULL DEFAULT '',
  updated_by         TEXT NOT NULL DEFAULT '',
  metadata_json      TEXT NOT NULL DEFAULT '{}',
  created_at         TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  updated_at         TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  deleted_at         TEXT
) STRICT;

CREATE TABLE IF NOT EXISTS session_attachments (
  id                TEXT PRIMARY KEY NOT NULL,
  workspace_id      TEXT NOT NULL DEFAULT '',
  session_id        TEXT NOT NULL DEFAULT '',
  filename          TEXT NOT NULL DEFAULT '',
  relative_path     TEXT NOT NULL DEFAULT '',
  content_type      TEXT NOT NULL DEFAULT '',
  size_bytes        INTEGER NOT NULL DEFAULT 0,
  sha256            TEXT NOT NULL DEFAULT '',
  storage_kind      TEXT NOT NULL DEFAULT 'local_file',
  cloud_object_key  TEXT NOT NULL DEFAULT '',
  source_type       TEXT NOT NULL DEFAULT '',
  source_id         TEXT NOT NULL DEFAULT '',
  metadata_json     TEXT NOT NULL DEFAULT '{}',
  created_at        TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  updated_at        TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  deleted_at        TEXT
) STRICT;

CREATE TABLE IF NOT EXISTS tags (
  id             TEXT PRIMARY KEY NOT NULL,
  workspace_id   TEXT NOT NULL DEFAULT '',
  owner_user_id  TEXT NOT NULL DEFAULT '',
  name           TEXT NOT NULL DEFAULT '',
  created_at     TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  updated_at     TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  deleted_at     TEXT
) STRICT;

CREATE TABLE IF NOT EXISTS session_tags (
  id            TEXT PRIMARY KEY NOT NULL,
  workspace_id  TEXT NOT NULL DEFAULT '',
  owner_user_id TEXT NOT NULL DEFAULT '',
  session_id    TEXT NOT NULL DEFAULT '',
  tag_id        TEXT NOT NULL DEFAULT '',
  created_at    TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  updated_at    TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  deleted_at    TEXT
) STRICT;

CREATE TABLE IF NOT EXISTS entity_mentions (
  id            TEXT PRIMARY KEY NOT NULL,
  workspace_id  TEXT NOT NULL DEFAULT '',
  owner_user_id TEXT NOT NULL DEFAULT '',
  source_type   TEXT NOT NULL DEFAULT '',
  source_id     TEXT NOT NULL DEFAULT '',
  target_type   TEXT NOT NULL DEFAULT '',
  target_id     TEXT NOT NULL DEFAULT '',
  created_at    TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  updated_at    TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  deleted_at    TEXT
) STRICT;

CREATE TABLE IF NOT EXISTS chat_groups (
  id             TEXT PRIMARY KEY NOT NULL,
  workspace_id   TEXT NOT NULL DEFAULT '',
  owner_user_id  TEXT NOT NULL DEFAULT '',
  title          TEXT NOT NULL DEFAULT '',
  created_at     TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  updated_at     TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  deleted_at     TEXT
) STRICT;

CREATE TABLE IF NOT EXISTS chat_messages (
  id             TEXT PRIMARY KEY NOT NULL,
  workspace_id   TEXT NOT NULL DEFAULT '',
  chat_group_id  TEXT NOT NULL DEFAULT '',
  owner_user_id  TEXT NOT NULL DEFAULT '',
  role           TEXT NOT NULL DEFAULT '',
  content        TEXT NOT NULL DEFAULT '',
  metadata_json  TEXT NOT NULL DEFAULT '{}',
  parts_json     TEXT NOT NULL DEFAULT '[]',
  status         TEXT NOT NULL DEFAULT 'ready',
  created_at     TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  updated_at     TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  deleted_at     TEXT
) STRICT;

CREATE TABLE IF NOT EXISTS daily_notes (
  id             TEXT PRIMARY KEY NOT NULL,
  workspace_id   TEXT NOT NULL DEFAULT '',
  owner_user_id  TEXT NOT NULL DEFAULT '',
  note_date      TEXT NOT NULL DEFAULT '',
  body_format    TEXT NOT NULL DEFAULT 'prosemirror_json',
  body           TEXT NOT NULL DEFAULT '',
  created_at     TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  updated_at     TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  deleted_at     TEXT
) STRICT;

CREATE TABLE IF NOT EXISTS app_settings (
  id          TEXT PRIMARY KEY NOT NULL,
  value_json  TEXT NOT NULL DEFAULT 'null',
  updated_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
) STRICT;

CREATE TABLE IF NOT EXISTS migration_import_runs (
  id                TEXT PRIMARY KEY NOT NULL,
  importer_version  INTEGER NOT NULL DEFAULT 1,
  source_root       TEXT NOT NULL DEFAULT '',
  dry_run           INTEGER NOT NULL DEFAULT 0,
  status            TEXT NOT NULL DEFAULT 'running',
  discovered_count  INTEGER NOT NULL DEFAULT 0,
  imported_count    INTEGER NOT NULL DEFAULT 0,
  skipped_count     INTEGER NOT NULL DEFAULT 0,
  conflict_count    INTEGER NOT NULL DEFAULT 0,
  error_count       INTEGER NOT NULL DEFAULT 0,
  started_at        TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  completed_at      TEXT,
  error             TEXT NOT NULL DEFAULT ''
) STRICT;

CREATE TABLE IF NOT EXISTS migration_import_items (
  id                TEXT PRIMARY KEY NOT NULL,
  run_id            TEXT NOT NULL DEFAULT '',
  source_path       TEXT NOT NULL DEFAULT '',
  source_kind       TEXT NOT NULL DEFAULT '',
  source_sha256     TEXT NOT NULL DEFAULT '',
  status            TEXT NOT NULL DEFAULT 'pending',
  discovered_count  INTEGER NOT NULL DEFAULT 0,
  imported_count    INTEGER NOT NULL DEFAULT 0,
  skipped_count     INTEGER NOT NULL DEFAULT 0,
  conflict_count    INTEGER NOT NULL DEFAULT 0,
  error             TEXT NOT NULL DEFAULT '',
  created_at        TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  completed_at      TEXT
) STRICT;

CREATE TABLE IF NOT EXISTS storage_migration_state (
  id                 TEXT PRIMARY KEY NOT NULL,
  importer_version   INTEGER NOT NULL DEFAULT 1,
  phase              TEXT NOT NULL DEFAULT 'shadow',
  latest_run_id      TEXT NOT NULL DEFAULT '',
  parity_verified    INTEGER NOT NULL DEFAULT 0,
  cutover_at         TEXT,
  rollback_until     TEXT,
  last_error         TEXT NOT NULL DEFAULT '',
  updated_at         TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
) STRICT;

CREATE INDEX IF NOT EXISTS idx_humans_organization_id ON humans(organization_id);
CREATE INDEX IF NOT EXISTS idx_humans_email ON humans(email);
CREATE INDEX IF NOT EXISTS idx_sessions_created_at ON sessions(created_at);
CREATE INDEX IF NOT EXISTS idx_sessions_folder_path ON sessions(folder_path);
CREATE INDEX IF NOT EXISTS idx_sessions_event_id ON sessions(event_id);
CREATE INDEX IF NOT EXISTS idx_session_documents_session_id ON session_documents(session_id);
CREATE INDEX IF NOT EXISTS idx_session_documents_kind ON session_documents(kind);
CREATE INDEX IF NOT EXISTS idx_transcripts_session_id ON transcripts(session_id);
CREATE INDEX IF NOT EXISTS idx_session_participants_session_id ON session_participants(session_id);
CREATE INDEX IF NOT EXISTS idx_session_participants_human_id ON session_participants(human_id);
CREATE INDEX IF NOT EXISTS idx_action_items_session_id ON action_items(session_id);
CREATE INDEX IF NOT EXISTS idx_action_items_source ON action_items(source_type, source_id);
CREATE INDEX IF NOT EXISTS idx_session_attachments_session_id ON session_attachments(session_id);
CREATE INDEX IF NOT EXISTS idx_tags_name ON tags(name);
CREATE INDEX IF NOT EXISTS idx_session_tags_session_id ON session_tags(session_id);
CREATE INDEX IF NOT EXISTS idx_session_tags_tag_id ON session_tags(tag_id);
CREATE INDEX IF NOT EXISTS idx_entity_mentions_source ON entity_mentions(source_type, source_id);
CREATE INDEX IF NOT EXISTS idx_entity_mentions_target ON entity_mentions(target_type, target_id);
CREATE INDEX IF NOT EXISTS idx_chat_messages_group_id ON chat_messages(chat_group_id, created_at);
CREATE INDEX IF NOT EXISTS idx_daily_notes_note_date ON daily_notes(note_date);
CREATE INDEX IF NOT EXISTS idx_migration_import_items_run_id ON migration_import_items(run_id);
CREATE INDEX IF NOT EXISTS idx_migration_import_items_source ON migration_import_items(source_path, source_sha256);

INSERT INTO storage_migration_state (id)
VALUES ('legacy_v1')
ON CONFLICT(id) DO NOTHING;
