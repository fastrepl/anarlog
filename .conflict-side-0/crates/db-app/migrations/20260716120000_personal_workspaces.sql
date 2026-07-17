CREATE TABLE IF NOT EXISTS workspaces (
  id             TEXT PRIMARY KEY NOT NULL,
  owner_user_id  TEXT NOT NULL DEFAULT '',
  kind           TEXT NOT NULL DEFAULT 'personal',
  name           TEXT NOT NULL DEFAULT '',
  created_at     TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  updated_at     TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  deleted_at     TEXT
) STRICT;

CREATE TABLE IF NOT EXISTS workspace_memberships (
  id            TEXT PRIMARY KEY NOT NULL,
  workspace_id  TEXT NOT NULL DEFAULT '',
  user_id       TEXT NOT NULL DEFAULT '',
  role          TEXT NOT NULL DEFAULT 'member',
  created_at    TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  updated_at    TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  deleted_at    TEXT,
  UNIQUE (workspace_id, user_id)
) STRICT;

CREATE INDEX IF NOT EXISTS idx_workspaces_owner_user_id
ON workspaces(owner_user_id);

CREATE INDEX IF NOT EXISTS idx_workspace_memberships_user_id
ON workspace_memberships(user_id);
