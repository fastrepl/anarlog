-- Local-only meeting chat notice claims and delivery evidence. Never
-- CloudSync-enabled: a claim is this device's record that a notice may already
-- be in a call's chat, and a delivered notice is not legal consent.
--
-- A claim is written before a notice can be sent and kept for seven days once
-- the notice was or may have been sent ('sent', 'unconfirmed'), or when the app
-- stopped before learning the outcome ('dispatching'). Only a notice that was
-- definitely not sent gives its claim back. claim_key is a SHA-256 of the
-- account and session, so neither id is stored here.
CREATE TABLE IF NOT EXISTS local_meeting_notice_claims (
  claim_key TEXT PRIMARY KEY NOT NULL CHECK (
    length(claim_key) = 64 AND claim_key NOT GLOB '*[^0-9a-f]*'
  ),
  capture_id TEXT NOT NULL CHECK (
    capture_id = trim(capture_id)
    AND length(capture_id) > 0
    AND length(capture_id) <= 128
  ),
  claimed_at_ms INTEGER NOT NULL CHECK (claimed_at_ms >= 0),
  state TEXT NOT NULL DEFAULT 'dispatching' CHECK (
    state IN ('dispatching', 'sent', 'unconfirmed')
  ),
  settled_at_ms INTEGER CHECK (settled_at_ms IS NULL OR settled_at_ms >= 0)
) STRICT;

-- Delivery evidence for notices sent through the claims above. Unlike
-- session_disclosure_attempts, whose shipped CHECK allows only sent, not_sent
-- and cancelled, this can record a notice that may have been sent.
CREATE TABLE IF NOT EXISTS session_disclosure_deliveries (
  id TEXT PRIMARY KEY NOT NULL CHECK (
    id = trim(id) AND length(id) > 0 AND length(id) <= 128
  ),
  session_id TEXT NOT NULL CHECK (
    session_id = trim(session_id)
    AND length(session_id) > 0
    AND length(session_id) <= 128
  ),
  capture_id TEXT NOT NULL CHECK (
    capture_id = trim(capture_id)
    AND length(capture_id) > 0
    AND length(capture_id) <= 128
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
  message_version TEXT NOT NULL DEFAULT 'anarlog-disclosure-v1' CHECK (
    length(message_version) BETWEEN 1 AND 64
  ),
  message TEXT NOT NULL DEFAULT '' CHECK (length(CAST(message AS BLOB)) <= 4096),
  delivery TEXT NOT NULL CHECK (
    delivery IN ('sent', 'not_sent', 'cancelled', 'unconfirmed')
  ),
  reason TEXT NOT NULL DEFAULT '' CHECK (length(reason) <= 64),
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
) STRICT;

CREATE INDEX IF NOT EXISTS idx_session_disclosure_deliveries_session
ON session_disclosure_deliveries (session_id, attempted_at);
