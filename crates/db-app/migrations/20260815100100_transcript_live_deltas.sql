CREATE TABLE IF NOT EXISTS transcript_live_state (
  transcript_id  TEXT PRIMARY KEY NOT NULL,
  next_sequence  INTEGER NOT NULL DEFAULT 0 CHECK (next_sequence >= 0),
  updated_at     TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
) STRICT;

CREATE TABLE IF NOT EXISTS transcript_live_deltas (
  id             TEXT PRIMARY KEY NOT NULL,
  transcript_id  TEXT NOT NULL DEFAULT ''
    REFERENCES transcript_live_state(transcript_id) ON DELETE CASCADE,
  sequence       INTEGER NOT NULL DEFAULT 0 CHECK (sequence >= 0),
  delta_json     TEXT NOT NULL DEFAULT '{"new_words":[],"replaced_ids":[],"partials":[]}'
    CHECK (json_valid(delta_json) AND json_type(delta_json) = 'object'),
  created_at     TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  UNIQUE (transcript_id, sequence)
) STRICT;

CREATE INDEX IF NOT EXISTS idx_transcript_live_deltas_transcript_sequence
ON transcript_live_deltas(transcript_id, sequence);
