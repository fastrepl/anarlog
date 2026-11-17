-- Transcript file flows: a voice memo whose saved audio (written by the harness) has no
-- transcript yet, and a blank note that offers Upload audio / Upload transcript.
INSERT INTO
  sessions (id, title, created_at, started_at, ended_at, folder_path)
VALUES
  (
    's-voice-memo',
    'Voice memo',
    strftime('%Y-%m-%dT%H:%M:%SZ', 'now', '-5 days'),
    strftime('%Y-%m-%dT%H:%M:%SZ', 'now', '-5 days'),
    strftime('%Y-%m-%dT%H:%M:%SZ', 'now', '-5 days', '+2 minutes'),
    ''
  ),
  (
    's-blank-upload',
    'Vendor call',
    strftime('%Y-%m-%dT%H:%M:%SZ', 'now', '-6 days'),
    strftime('%Y-%m-%dT%H:%M:%SZ', 'now', '-6 days'),
    '',
    ''
  );
