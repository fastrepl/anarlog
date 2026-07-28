// Interim TS mirror of crates/db-app migrations for the mobile client.
// The UniFFI mobile-bridge will own the schema later; this local DB is
// disposable — cloud sync repopulates it.
import type { SQLiteDatabase } from "expo-sqlite";

import { captureOperationalError } from "@/lib/error-reporting";

const CANONICAL_TABLES = [
  `CREATE TABLE IF NOT EXISTS sessions (
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
) STRICT`,
  `CREATE TABLE IF NOT EXISTS session_documents (
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
) STRICT`,
  `CREATE TABLE IF NOT EXISTS transcripts (
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
) STRICT`,
  `CREATE TABLE IF NOT EXISTS session_participants (
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
) STRICT`,
  `CREATE TABLE IF NOT EXISTS session_attachments (
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
  deleted_at        TEXT,
  cloud_sync_enabled INTEGER NOT NULL DEFAULT 0 CHECK (cloud_sync_enabled IN (0, 1))
) STRICT`,
  `CREATE TABLE IF NOT EXISTS humans (
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
) STRICT`,
  `CREATE TABLE IF NOT EXISTS organizations (
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
) STRICT`,
  `CREATE TABLE IF NOT EXISTS tags (
  id             TEXT PRIMARY KEY NOT NULL,
  workspace_id   TEXT NOT NULL DEFAULT '',
  owner_user_id  TEXT NOT NULL DEFAULT '',
  name           TEXT NOT NULL DEFAULT '',
  created_at     TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  updated_at     TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  deleted_at     TEXT
) STRICT`,
  `CREATE TABLE IF NOT EXISTS session_tags (
  id            TEXT PRIMARY KEY NOT NULL,
  workspace_id  TEXT NOT NULL DEFAULT '',
  owner_user_id TEXT NOT NULL DEFAULT '',
  session_id    TEXT NOT NULL DEFAULT '',
  tag_id        TEXT NOT NULL DEFAULT '',
  created_at    TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  updated_at    TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  deleted_at    TEXT
) STRICT`,
  `CREATE TABLE IF NOT EXISTS action_items (
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
) STRICT`,
  `CREATE TABLE IF NOT EXISTS entity_mentions (
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
) STRICT`,
  `CREATE TABLE IF NOT EXISTS app_settings (
  id          TEXT PRIMARY KEY NOT NULL,
  value_json  TEXT NOT NULL DEFAULT 'null',
  updated_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
) STRICT`,
];

const CANONICAL_INDEXES = [
  "CREATE INDEX IF NOT EXISTS idx_humans_organization_id ON humans(organization_id)",
  "CREATE INDEX IF NOT EXISTS idx_humans_email ON humans(email)",
  "CREATE INDEX IF NOT EXISTS idx_sessions_created_at ON sessions(created_at)",
  "CREATE INDEX IF NOT EXISTS idx_sessions_folder_path ON sessions(folder_path)",
  "CREATE INDEX IF NOT EXISTS idx_sessions_event_id ON sessions(event_id)",
  "CREATE INDEX IF NOT EXISTS idx_session_documents_session_id ON session_documents(session_id)",
  "CREATE INDEX IF NOT EXISTS idx_session_documents_kind ON session_documents(kind)",
  "CREATE INDEX IF NOT EXISTS idx_transcripts_session_id ON transcripts(session_id)",
  "CREATE INDEX IF NOT EXISTS idx_session_participants_session_id ON session_participants(session_id)",
  "CREATE INDEX IF NOT EXISTS idx_session_participants_human_id ON session_participants(human_id)",
  "CREATE INDEX IF NOT EXISTS idx_action_items_session_id ON action_items(session_id)",
  "CREATE INDEX IF NOT EXISTS idx_action_items_source ON action_items(source_type, source_id)",
  "CREATE INDEX IF NOT EXISTS idx_session_attachments_session_id ON session_attachments(session_id)",
  "CREATE INDEX IF NOT EXISTS idx_tags_name ON tags(name)",
  "CREATE INDEX IF NOT EXISTS idx_session_tags_session_id ON session_tags(session_id)",
  "CREATE INDEX IF NOT EXISTS idx_session_tags_tag_id ON session_tags(tag_id)",
  "CREATE INDEX IF NOT EXISTS idx_entity_mentions_source ON entity_mentions(source_type, source_id)",
  "CREATE INDEX IF NOT EXISTS idx_entity_mentions_target ON entity_mentions(target_type, target_id)",
];

const CALENDAR_TABLES = [
  `CREATE TABLE IF NOT EXISTS calendars (
  id                    TEXT PRIMARY KEY NOT NULL,
  tracking_id_calendar  TEXT NOT NULL DEFAULT '',
  name                  TEXT NOT NULL DEFAULT '',
  enabled               INTEGER NOT NULL DEFAULT 0,
  provider              TEXT NOT NULL DEFAULT '',
  source                TEXT NOT NULL DEFAULT '',
  color                 TEXT NOT NULL DEFAULT '#888',
  connection_id         TEXT NOT NULL DEFAULT '',
  created_at            TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
  updated_at            TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
  deleted_at            TEXT
)`,
  `CREATE TABLE IF NOT EXISTS events (
  id                    TEXT PRIMARY KEY NOT NULL,
  tracking_id_event     TEXT NOT NULL DEFAULT '',
  calendar_id           TEXT NOT NULL DEFAULT '',
  title                 TEXT NOT NULL DEFAULT '',
  started_at            TEXT NOT NULL DEFAULT '',
  ended_at              TEXT NOT NULL DEFAULT '',
  location              TEXT NOT NULL DEFAULT '',
  meeting_link          TEXT NOT NULL DEFAULT '',
  description           TEXT NOT NULL DEFAULT '',
  note                  TEXT NOT NULL DEFAULT '',
  recurrence_series_id  TEXT NOT NULL DEFAULT '',
  has_recurrence_rules  INTEGER NOT NULL DEFAULT 0,
  is_all_day            INTEGER NOT NULL DEFAULT 0,
  provider              TEXT NOT NULL DEFAULT '',
  participants_json     TEXT,
  created_at            TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
  updated_at            TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
  deleted_at            TEXT
)`,
  "CREATE INDEX IF NOT EXISTS idx_events_calendar_id ON events(calendar_id)",
  "CREATE INDEX IF NOT EXISTS idx_events_started_at ON events(started_at)",
  "CREATE INDEX IF NOT EXISTS idx_calendars_provider ON calendars(provider)",
];

const WORKSPACE_TABLES = [
  `CREATE TABLE IF NOT EXISTS workspaces (
  id             TEXT PRIMARY KEY NOT NULL,
  owner_user_id  TEXT NOT NULL DEFAULT '',
  kind           TEXT NOT NULL DEFAULT 'personal',
  name           TEXT NOT NULL DEFAULT '',
  created_at     TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  updated_at     TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  deleted_at     TEXT
) STRICT`,
  `CREATE TABLE IF NOT EXISTS workspace_memberships (
  id            TEXT PRIMARY KEY NOT NULL,
  workspace_id  TEXT NOT NULL DEFAULT '',
  user_id       TEXT NOT NULL DEFAULT '',
  role          TEXT NOT NULL DEFAULT 'member',
  created_at    TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  updated_at    TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  deleted_at    TEXT,
  UNIQUE (workspace_id, user_id)
) STRICT`,
  "CREATE INDEX IF NOT EXISTS idx_workspaces_owner_user_id ON workspaces(owner_user_id)",
  "CREATE INDEX IF NOT EXISTS idx_workspace_memberships_user_id ON workspace_memberships(user_id)",
];

const ATTACHMENT_STATE_TABLES = [
  `CREATE TABLE IF NOT EXISTS attachment_local_state (
  attachment_id  TEXT PRIMARY KEY NOT NULL,
  session_id     TEXT NOT NULL DEFAULT '',
  relative_path  TEXT NOT NULL DEFAULT '',
  availability   TEXT NOT NULL DEFAULT 'present'
                 CHECK (availability IN ('present', 'absent')),
  updated_at     TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
) STRICT`,
  `CREATE INDEX IF NOT EXISTS idx_attachment_local_state_session_id
ON attachment_local_state(session_id)`,
  `CREATE TABLE IF NOT EXISTS attachment_transfer_jobs (
  id                  TEXT PRIMARY KEY NOT NULL
                      CHECK (id <> '' AND length(id) <= 512),
  attachment_id       TEXT NOT NULL
                      CHECK (attachment_id <> '' AND length(attachment_id) <= 512),
  session_id          TEXT NOT NULL
                      CHECK (session_id <> '' AND length(session_id) <= 512),
  workspace_id        TEXT NOT NULL
                      CHECK (workspace_id <> '' AND length(workspace_id) <= 512),
  direction           TEXT NOT NULL
                      CHECK (direction IN ('upload', 'download', 'delete')),
  expected_sha256     TEXT NOT NULL
                      CHECK (length(expected_sha256) = 64
                             AND expected_sha256 NOT GLOB '*[^0-9a-f]*'),
  expected_size_bytes INTEGER NOT NULL DEFAULT 0
                      CHECK (expected_size_bytes >= 0 AND expected_size_bytes <= 536870912),
  ciphertext_sha256   TEXT NOT NULL DEFAULT ''
                      CHECK (ciphertext_sha256 = ''
                             OR (length(ciphertext_sha256) = 64
                                 AND ciphertext_sha256 NOT GLOB '*[^0-9a-f]*')),
  ciphertext_size_bytes INTEGER NOT NULL DEFAULT 0
                        CHECK (ciphertext_size_bytes >= 0 AND ciphertext_size_bytes <= 545259520),
  remote_object_id    TEXT NOT NULL DEFAULT '' CHECK (length(remote_object_id) <= 1024),
  object_key          TEXT NOT NULL DEFAULT '' CHECK (length(object_key) <= 2048),
  cache_id            TEXT NOT NULL DEFAULT '' CHECK (length(cache_id) <= 1024),
  phase               TEXT NOT NULL DEFAULT 'queued'
                      CHECK (phase IN ('queued','preparing','ready','transferring',
                                       'finalizing','retry_wait','failed','completed')),
  attempt_count       INTEGER NOT NULL DEFAULT 0 CHECK (attempt_count >= 0),
  next_attempt_at     TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  last_attempt_at     TEXT,
  last_error          TEXT NOT NULL DEFAULT '' CHECK (length(last_error) <= 2048),
  created_at          TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  updated_at          TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  completed_at        TEXT,
  CHECK (direction = 'upload' OR object_key <> ''),
  CHECK ((ciphertext_sha256 = '' AND ciphertext_size_bytes = 0)
      OR (ciphertext_sha256 <> '' AND ciphertext_size_bytes > 0))
) STRICT`,
  `CREATE UNIQUE INDEX IF NOT EXISTS idx_attachment_transfer_jobs_live_version
ON attachment_transfer_jobs (attachment_id, expected_sha256, expected_size_bytes)
WHERE direction IN ('upload', 'download') AND phase <> 'completed'`,
  `CREATE UNIQUE INDEX IF NOT EXISTS idx_attachment_transfer_jobs_delete_object
ON attachment_transfer_jobs(object_key)
WHERE direction = 'delete' AND phase <> 'completed'`,
  `CREATE UNIQUE INDEX IF NOT EXISTS idx_attachment_transfer_jobs_upload_object_id
ON attachment_transfer_jobs(remote_object_id)
WHERE direction = 'upload' AND remote_object_id <> '' AND phase <> 'completed'`,
  `CREATE UNIQUE INDEX IF NOT EXISTS idx_attachment_transfer_jobs_delete_object_id
ON attachment_transfer_jobs(remote_object_id)
WHERE direction = 'delete' AND remote_object_id <> '' AND phase <> 'completed'`,
  `CREATE UNIQUE INDEX IF NOT EXISTS idx_attachment_transfer_jobs_cache_id
ON attachment_transfer_jobs(cache_id) WHERE cache_id <> ''`,
  `CREATE INDEX IF NOT EXISTS idx_attachment_transfer_jobs_due
ON attachment_transfer_jobs(phase, next_attempt_at, created_at)`,
  `CREATE INDEX IF NOT EXISTS idx_attachment_transfer_jobs_attachment_id
ON attachment_transfer_jobs(attachment_id)`,
  `CREATE INDEX IF NOT EXISTS idx_attachment_transfer_jobs_session_id
ON attachment_transfer_jobs(session_id)`,
];

const PROJECTION_INDEXES = [
  "CREATE INDEX IF NOT EXISTS idx_sessions_workspace_id_id ON sessions(workspace_id, id)",
  "CREATE INDEX IF NOT EXISTS idx_workspace_memberships_user_deleted_id ON workspace_memberships(user_id, deleted_at, id)",
];

export const MIGRATIONS: { version: number; statements: string[] }[] = [
  {
    version: 1,
    statements: [
      ...CANONICAL_TABLES,
      ...CANONICAL_INDEXES,
      ...CALENDAR_TABLES,
      ...WORKSPACE_TABLES,
      ...ATTACHMENT_STATE_TABLES,
      ...PROJECTION_INDEXES,
    ],
  },
];

export async function migrate(db: SQLiteDatabase): Promise<void> {
  const row = await db.getFirstAsync<{ user_version: number }>(
    "PRAGMA user_version",
  );
  let current = row?.user_version ?? 0;
  for (const migration of MIGRATIONS) {
    if (migration.version <= current) {
      continue;
    }
    await db.execAsync("BEGIN");
    try {
      for (const statement of migration.statements) {
        await db.execAsync(statement);
      }
      await db.execAsync(`PRAGMA user_version = ${migration.version}`);
      await db.execAsync("COMMIT");
    } catch (error) {
      try {
        await db.execAsync("ROLLBACK");
      } catch (rollbackError) {
        captureOperationalError(rollbackError, {
          operation: "database_migration_rollback",
          level: "warning",
        });
      }
      throw error;
    }
    current = migration.version;
  }
}
