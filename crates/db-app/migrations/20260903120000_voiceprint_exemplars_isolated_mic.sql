-- Allow 'isolated_mic_capture' as a confirmation source: with headphone output the direct-mic
-- channel can only carry the local user, so those spans confirm the owner's voiceprint without
-- a manual speaker assignment. SQLite cannot widen a CHECK in place, so rebuild this
-- local-only table. Older builds keep working: the columns are unchanged and their inserts
-- still satisfy the wider constraint.
CREATE TABLE voiceprint_exemplars_next (
  id                    TEXT PRIMARY KEY NOT NULL CHECK (
    id = trim(id) AND length(id) > 0
  ),
  workspace_id          TEXT NOT NULL DEFAULT '' CHECK (
    workspace_id = trim(workspace_id) AND length(workspace_id) > 0
  ),
  human_id              TEXT NOT NULL DEFAULT '' CHECK (
    human_id = trim(human_id) AND length(human_id) > 0
  ),
  keyring_scope          TEXT NOT NULL DEFAULT 'voiceprint_exemplars' CHECK (
    keyring_scope = 'voiceprint_exemplars'
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
  confirmation_source   TEXT NOT NULL DEFAULT '' CHECK (
    confirmation_source IN (
      'accessibility_active_speaker',
      'manual_speaker_assignment',
      'isolated_mic_capture'
    )
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
  source_start_ms       INTEGER NOT NULL DEFAULT 0 CHECK (source_start_ms >= 0),
  source_end_ms         INTEGER NOT NULL DEFAULT 0 CHECK (source_end_ms > source_start_ms),
  quality_score         REAL NOT NULL DEFAULT 0 CHECK (quality_score BETWEEN 0.0 AND 1.0),
  label_confidence      REAL NOT NULL DEFAULT 0 CHECK (label_confidence BETWEEN 0.0 AND 1.0),
  confirmed_at          TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  created_at            TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  updated_at            TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  deleted_at            TEXT,
  UNIQUE (keyring_scope, keyring_key)
) STRICT;

INSERT INTO voiceprint_exemplars_next (
  id, workspace_id, human_id, keyring_scope, keyring_key, sync_scope,
  model_provider, model_version, capture_domain, confirmation_source,
  source_session_id, source_transcript_id, source_attachment_id,
  source_speaker_label, source_start_ms, source_end_ms, quality_score,
  label_confidence, confirmed_at, created_at, updated_at, deleted_at
)
SELECT
  id, workspace_id, human_id, keyring_scope, keyring_key, sync_scope,
  model_provider, model_version, capture_domain, confirmation_source,
  source_session_id, source_transcript_id, source_attachment_id,
  source_speaker_label, source_start_ms, source_end_ms, quality_score,
  label_confidence, confirmed_at, created_at, updated_at, deleted_at
FROM voiceprint_exemplars;

DROP TABLE voiceprint_exemplars;

ALTER TABLE voiceprint_exemplars_next RENAME TO voiceprint_exemplars;

CREATE INDEX IF NOT EXISTS idx_voiceprint_exemplars_human
ON voiceprint_exemplars(workspace_id, human_id, created_at);

CREATE INDEX IF NOT EXISTS idx_voiceprint_exemplars_source_session
ON voiceprint_exemplars(source_session_id, source_start_ms);
