CREATE INDEX IF NOT EXISTS idx_sessions_workspace_id_id
ON sessions(workspace_id, id);

CREATE INDEX IF NOT EXISTS idx_cloudsync_session_evictions_workspace_id_session_id
ON cloudsync_session_evictions(workspace_id, session_id);

CREATE INDEX IF NOT EXISTS idx_workspace_memberships_user_deleted_id
ON workspace_memberships(user_id, deleted_at, id);
