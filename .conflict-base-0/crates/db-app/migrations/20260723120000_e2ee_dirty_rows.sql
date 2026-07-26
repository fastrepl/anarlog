CREATE TABLE IF NOT EXISTS e2ee_dirty_rows (
  workspace_id  TEXT NOT NULL,
  table_name    TEXT NOT NULL,
  row_id        TEXT NOT NULL,
  generation    INTEGER NOT NULL DEFAULT 1 CHECK (generation > 0),
  PRIMARY KEY (workspace_id, table_name, row_id)
) STRICT;

CREATE TABLE IF NOT EXISTS e2ee_apply_guard (
  workspace_id  TEXT NOT NULL,
  table_name    TEXT NOT NULL,
  row_id        TEXT NOT NULL,
  PRIMARY KEY (workspace_id, table_name, row_id)
) STRICT;

INSERT INTO e2ee_dirty_rows (workspace_id, table_name, row_id)
SELECT workspace_id, 'action_items', id
FROM action_items;

INSERT INTO e2ee_dirty_rows (workspace_id, table_name, row_id)
SELECT DISTINCT state.workspace_id, state.table_name, state.row_id
FROM e2ee_local_state AS state
WHERE state.table_name = 'action_items'
  AND state.field_name = '$row'
  AND NOT EXISTS (
    SELECT 1
    FROM action_items AS domain
    WHERE domain.workspace_id = state.workspace_id
      AND domain.id = state.row_id
  )
ON CONFLICT (workspace_id, table_name, row_id) DO NOTHING;

INSERT INTO e2ee_dirty_rows (workspace_id, table_name, row_id)
SELECT workspace_id, 'humans', id
FROM humans;

INSERT INTO e2ee_dirty_rows (workspace_id, table_name, row_id)
SELECT DISTINCT state.workspace_id, state.table_name, state.row_id
FROM e2ee_local_state AS state
WHERE state.table_name = 'humans'
  AND state.field_name = '$row'
  AND NOT EXISTS (
    SELECT 1
    FROM humans AS domain
    WHERE domain.workspace_id = state.workspace_id
      AND domain.id = state.row_id
  )
ON CONFLICT (workspace_id, table_name, row_id) DO NOTHING;

INSERT INTO e2ee_dirty_rows (workspace_id, table_name, row_id)
SELECT workspace_id, 'organizations', id
FROM organizations;

INSERT INTO e2ee_dirty_rows (workspace_id, table_name, row_id)
SELECT DISTINCT state.workspace_id, state.table_name, state.row_id
FROM e2ee_local_state AS state
WHERE state.table_name = 'organizations'
  AND state.field_name = '$row'
  AND NOT EXISTS (
    SELECT 1
    FROM organizations AS domain
    WHERE domain.workspace_id = state.workspace_id
      AND domain.id = state.row_id
  )
ON CONFLICT (workspace_id, table_name, row_id) DO NOTHING;

INSERT INTO e2ee_dirty_rows (workspace_id, table_name, row_id)
SELECT workspace_id, 'session_attachments', id
FROM session_attachments;

INSERT INTO e2ee_dirty_rows (workspace_id, table_name, row_id)
SELECT DISTINCT state.workspace_id, state.table_name, state.row_id
FROM e2ee_local_state AS state
WHERE state.table_name = 'session_attachments'
  AND state.field_name = '$row'
  AND NOT EXISTS (
    SELECT 1
    FROM session_attachments AS domain
    WHERE domain.workspace_id = state.workspace_id
      AND domain.id = state.row_id
  )
ON CONFLICT (workspace_id, table_name, row_id) DO NOTHING;

INSERT INTO e2ee_dirty_rows (workspace_id, table_name, row_id)
SELECT workspace_id, 'session_documents', id
FROM session_documents;

INSERT INTO e2ee_dirty_rows (workspace_id, table_name, row_id)
SELECT DISTINCT state.workspace_id, state.table_name, state.row_id
FROM e2ee_local_state AS state
WHERE state.table_name = 'session_documents'
  AND state.field_name = '$row'
  AND NOT EXISTS (
    SELECT 1
    FROM session_documents AS domain
    WHERE domain.workspace_id = state.workspace_id
      AND domain.id = state.row_id
  )
ON CONFLICT (workspace_id, table_name, row_id) DO NOTHING;

INSERT INTO e2ee_dirty_rows (workspace_id, table_name, row_id)
SELECT workspace_id, 'session_participants', id
FROM session_participants;

INSERT INTO e2ee_dirty_rows (workspace_id, table_name, row_id)
SELECT DISTINCT state.workspace_id, state.table_name, state.row_id
FROM e2ee_local_state AS state
WHERE state.table_name = 'session_participants'
  AND state.field_name = '$row'
  AND NOT EXISTS (
    SELECT 1
    FROM session_participants AS domain
    WHERE domain.workspace_id = state.workspace_id
      AND domain.id = state.row_id
  )
ON CONFLICT (workspace_id, table_name, row_id) DO NOTHING;

INSERT INTO e2ee_dirty_rows (workspace_id, table_name, row_id)
SELECT workspace_id, 'sessions', id
FROM sessions;

INSERT INTO e2ee_dirty_rows (workspace_id, table_name, row_id)
SELECT DISTINCT state.workspace_id, state.table_name, state.row_id
FROM e2ee_local_state AS state
WHERE state.table_name = 'sessions'
  AND state.field_name = '$row'
  AND NOT EXISTS (
    SELECT 1
    FROM sessions AS domain
    WHERE domain.workspace_id = state.workspace_id
      AND domain.id = state.row_id
  )
ON CONFLICT (workspace_id, table_name, row_id) DO NOTHING;

INSERT INTO e2ee_dirty_rows (workspace_id, table_name, row_id)
SELECT workspace_id, 'transcripts', id
FROM transcripts;

INSERT INTO e2ee_dirty_rows (workspace_id, table_name, row_id)
SELECT DISTINCT state.workspace_id, state.table_name, state.row_id
FROM e2ee_local_state AS state
WHERE state.table_name = 'transcripts'
  AND state.field_name = '$row'
  AND NOT EXISTS (
    SELECT 1
    FROM transcripts AS domain
    WHERE domain.workspace_id = state.workspace_id
      AND domain.id = state.row_id
  )
ON CONFLICT (workspace_id, table_name, row_id) DO NOTHING;
