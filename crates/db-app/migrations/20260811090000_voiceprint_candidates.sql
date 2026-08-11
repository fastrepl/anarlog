CREATE TABLE IF NOT EXISTS voiceprint_candidates (
  id                    TEXT PRIMARY KEY NOT NULL CHECK (
    id = trim(id) AND length(id) > 0
  ),
  workspace_id          TEXT NOT NULL DEFAULT '' CHECK (
    workspace_id = trim(workspace_id) AND length(workspace_id) > 0
  ),
  keyring_scope          TEXT NOT NULL DEFAULT 'voiceprint_candidates' CHECK (
    keyring_scope = 'voiceprint_candidates'
  ),
  keyring_key            TEXT NOT NULL DEFAULT '' CHECK (keyring_key = id),
  sync_scope             TEXT NOT NULL DEFAULT 'local_only' CHECK (
    sync_scope = 'local_only'
  ),
  model_provider        TEXT NOT NULL DEFAULT '' CHECK (
    model_provider = trim(model_provider) AND length(model_provider) > 0
  ),
  model_version         TEXT NOT NULL DEFAULT '' CHECK (
    model_version = trim(model_version) AND length(model_version) > 0
  ),
  capture_domain        TEXT NOT NULL DEFAULT '' CHECK (
    capture_domain = trim(capture_domain) AND length(capture_domain) > 0
  ),
  source_session_id     TEXT NOT NULL DEFAULT '' CHECK (
    source_session_id = trim(source_session_id) AND length(source_session_id) > 0
  ),
  source_transcript_id  TEXT NOT NULL DEFAULT '' CHECK (
    source_transcript_id = trim(source_transcript_id) AND length(source_transcript_id) > 0
  ),
  source_attachment_id  TEXT NOT NULL DEFAULT '' CHECK (
    source_attachment_id = trim(source_attachment_id) AND length(source_attachment_id) > 0
  ),
  source_speaker_label  TEXT NOT NULL DEFAULT '' CHECK (
    source_speaker_label = trim(source_speaker_label)
    AND length(source_speaker_label) > 0
    AND length(CAST(source_speaker_label AS BLOB)) <= 1024
  ),
  speaker_channel       INTEGER NOT NULL DEFAULT 0 CHECK (speaker_channel >= 0),
  speaker_index         INTEGER CHECK (speaker_index IS NULL OR speaker_index >= 0),
  source_start_ms       INTEGER NOT NULL DEFAULT 0 CHECK (source_start_ms >= 0),
  source_end_ms         INTEGER NOT NULL DEFAULT 0 CHECK (source_end_ms > source_start_ms),
  quality_score         REAL NOT NULL DEFAULT 0 CHECK (quality_score BETWEEN 0.0 AND 1.0),
  expires_at            TEXT NOT NULL DEFAULT '' CHECK (
    expires_at = trim(expires_at) AND length(expires_at) > 0
  ),
  created_at            TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  updated_at            TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  deleted_at            TEXT,
  UNIQUE (keyring_scope, keyring_key)
) STRICT;

CREATE INDEX IF NOT EXISTS idx_voiceprint_candidates_speaker
ON voiceprint_candidates(
  workspace_id, source_transcript_id, speaker_channel, speaker_index
);

CREATE INDEX IF NOT EXISTS idx_voiceprint_candidates_expiry
ON voiceprint_candidates(expires_at)
WHERE deleted_at IS NULL;
