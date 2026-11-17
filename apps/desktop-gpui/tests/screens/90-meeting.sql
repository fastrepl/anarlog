-- Meeting fixtures: a note being recorded right now (the harness sets its live state) and a
-- finished recording with a transcript; the harness writes its saved audio into the vault.
INSERT INTO
  sessions (
    id,
    title,
    created_at,
    started_at,
    ended_at,
    folder_path
  )
VALUES
  (
    's-standup',
    'Design standup',
    strftime('%Y-%m-%dT%H:%M:%SZ', 'now', '-3 minutes'),
    strftime('%Y-%m-%dT%H:%M:%SZ', 'now', '-3 minutes'),
    '',
    ''
  ),
  (
    's-retro',
    'Sprint retro',
    strftime('%Y-%m-%dT%H:%M:%SZ', 'now', '-2 days'),
    strftime('%Y-%m-%dT%H:%M:%SZ', 'now', '-2 days'),
    strftime(
      '%Y-%m-%dT%H:%M:%SZ',
      'now',
      '-2 days',
      '+25 minutes'
    ),
    ''
  );
INSERT INTO
  session_documents (
    id,
    session_id,
    kind,
    title,
    body_format,
    body,
    sort_order
  )
VALUES
  (
    's-standup',
    's-standup',
    'note',
    '',
    'prosemirror_json',
    '{"type":"doc","content":[{"type":"paragraph","content":[{"type":"text","text":"Walk through the launch checklist."}]}]}',
    0
  ),
  (
    's-retro',
    's-retro',
    'note',
    '',
    'prosemirror_json',
    '{"type":"doc","content":[{"type":"paragraph","content":[{"type":"text","text":"Keep the release train weekly."}]}]}',
    0
  );
INSERT INTO
  transcripts (
    id,
    session_id,
    started_at_ms,
    ended_at_ms,
    words_json
  )
VALUES
  (
    't-retro',
    's-retro',
    0,
    60000,
    '[
    {"id":"r1","text":"What","start_ms":1000,"end_ms":1200,"channel":0},
    {"id":"r2","text":" went","start_ms":1200,"end_ms":1400,"channel":0},
    {"id":"r3","text":" well","start_ms":1400,"end_ms":1700,"channel":0},
    {"id":"r4","text":" this","start_ms":1700,"end_ms":1900,"channel":0},
    {"id":"r5","text":" sprint?","start_ms":1900,"end_ms":2400,"channel":0},
    {"id":"r6","text":"The","start_ms":4000,"end_ms":4200,"channel":1},
    {"id":"r7","text":" release","start_ms":4200,"end_ms":4600,"channel":1},
    {"id":"r8","text":" went","start_ms":4600,"end_ms":4800,"channel":1},
    {"id":"r9","text":" out","start_ms":4800,"end_ms":5000,"channel":1},
    {"id":"r10","text":" on","start_ms":5000,"end_ms":5100,"channel":1},
    {"id":"r11","text":" time.","start_ms":5100,"end_ms":5600,"channel":1},
    {"id":"r12","text":"Let''s","start_ms":31000,"end_ms":31300,"channel":0},
    {"id":"r13","text":" keep","start_ms":31300,"end_ms":31600,"channel":0},
    {"id":"r14","text":" the","start_ms":31600,"end_ms":31700,"channel":0},
    {"id":"r15","text":" train","start_ms":31700,"end_ms":32000,"channel":0},
    {"id":"r16","text":" weekly.","start_ms":32000,"end_ms":32600,"channel":0}]'
  );
