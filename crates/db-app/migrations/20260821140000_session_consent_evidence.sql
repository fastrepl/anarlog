-- Local-only disclosure transport evidence and per-participant consent
-- state. These tables must never be CloudSync-enabled: a sent disclosure
-- is not legal consent and must not replicate as if it were.
CREATE TABLE IF NOT EXISTS session_disclosure_attempts (
  id TEXT PRIMARY KEY CHECK (
    id = trim(id) AND length(id) > 0 AND length(id) <= 128
  ),
  session_id TEXT NOT NULL CHECK (
    session_id = trim(session_id)
    AND length(session_id) > 0
    AND length(session_id) <= 128
  ),
  attempted_at TEXT NOT NULL CHECK (
    attempted_at = trim(attempted_at) AND length(attempted_at) > 0
  ),
  platform TEXT NOT NULL DEFAULT 'unknown' CHECK (
    platform IN (
      'slack_huddle',
      'zoom',
      'google_meet',
      'teams',
      'webex',
      'browser',
      'unknown'
    )
  ),
  surface TEXT NOT NULL DEFAULT '' CHECK (length(surface) <= 128),
  message_version TEXT NOT NULL DEFAULT 'anarlog-disclosure-v1' CHECK (
    length(message_version) BETWEEN 1 AND 64
  ),
  message TEXT NOT NULL DEFAULT '' CHECK (length(CAST(message AS BLOB)) <= 4096),
  delivery TEXT NOT NULL CHECK (delivery IN ('sent', 'not_sent', 'cancelled')),
  failure_reason TEXT NOT NULL DEFAULT '' CHECK (
    length(CAST(failure_reason AS BLOB)) <= 2048
  ),
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
) STRICT;

CREATE INDEX IF NOT EXISTS idx_session_disclosure_attempts_session
ON session_disclosure_attempts (session_id, attempted_at);

CREATE TABLE IF NOT EXISTS session_participant_consent (
  session_id TEXT NOT NULL CHECK (
    session_id = trim(session_id)
    AND length(session_id) > 0
    AND length(session_id) <= 128
  ),
  participant_key TEXT NOT NULL CHECK (
    participant_key = trim(participant_key)
    AND length(participant_key) > 0
    AND length(participant_key) <= 256
  ),
  status TEXT NOT NULL CHECK (status IN ('unknown', 'consented', 'declined')),
  source TEXT NOT NULL CHECK (
    source IN ('explicit_chat_reply', 'explicit_ui', 'unseen')
  ),
  updated_at TEXT NOT NULL CHECK (
    updated_at = trim(updated_at) AND length(updated_at) > 0
  ),
  PRIMARY KEY (session_id, participant_key)
) STRICT;
