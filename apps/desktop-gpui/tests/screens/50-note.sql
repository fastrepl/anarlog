-- Open-note fixtures: every editor block and mark, and a finished meeting with a summary and a
-- transcript so the note view switcher (Summary / Memos / Transcript) renders.
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
    's-typography',
    'Editor typography',
    strftime('%Y-%m-%dT%H:%M:%SZ', 'now', '-45 days'),
    '',
    '',
    ''
  ),
  (
    's-critique',
    'Design critique',
    strftime('%Y-%m-%dT%H:%M:%SZ', 'now', '-50 days'),
    strftime('%Y-%m-%dT%H:%M:%SZ', 'now', '-50 days'),
    strftime(
      '%Y-%m-%dT%H:%M:%SZ',
      'now',
      '-50 days',
      '+30 minutes'
    ),
    'Research'
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
    's-typography',
    's-typography',
    'note',
    '',
    'prosemirror_json',
    '{"type":"doc","content":[
    {"type":"heading","attrs":{"level":1},"content":[{"type":"text","text":"Heading one"}]},
    {"type":"heading","attrs":{"level":2},"content":[{"type":"text","text":"Heading two"}]},
    {"type":"heading","attrs":{"level":3},"content":[{"type":"text","text":"Heading three"}]},
    {"type":"paragraph","content":[
      {"type":"text","marks":[{"type":"bold"}],"text":"Bold"},{"type":"text","text":", "},
      {"type":"text","marks":[{"type":"italic"}],"text":"italic"},{"type":"text","text":", "},
      {"type":"text","marks":[{"type":"strike"}],"text":"struck"},{"type":"text","text":", "},
      {"type":"text","marks":[{"type":"underline"}],"text":"underlined"},{"type":"text","text":", "},
      {"type":"text","marks":[{"type":"code"}],"text":"inline code"},{"type":"text","text":", "},
      {"type":"text","marks":[{"type":"link","attrs":{"href":"https://example.com"}}],"text":"a link"},{"type":"text","text":" and "},
      {"type":"text","marks":[{"type":"highlight"}],"text":"highlighted"},{"type":"text","text":" text."}]},
    {"type":"bulletList","content":[
      {"type":"listItem","content":[{"type":"paragraph","content":[{"type":"text","text":"First level"}]},
        {"type":"bulletList","content":[
          {"type":"listItem","content":[{"type":"paragraph","content":[{"type":"text","text":"Second level"}]},
            {"type":"bulletList","content":[
              {"type":"listItem","content":[{"type":"paragraph","content":[{"type":"text","text":"Third level"}]}]}]}]}]}]}]},
    {"type":"orderedList","attrs":{"start":1},"content":[
      {"type":"listItem","content":[{"type":"paragraph","content":[{"type":"text","text":"Ordered one"}]},
        {"type":"orderedList","attrs":{"start":1},"content":[
          {"type":"listItem","content":[{"type":"paragraph","content":[{"type":"text","text":"Nested ordered"}]}]}]}]},
      {"type":"listItem","content":[{"type":"paragraph","content":[{"type":"text","text":"Ordered two"}]}]}]},
    {"type":"taskList","content":[
      {"type":"taskItem","attrs":{"status":"done","checked":true},"content":[{"type":"paragraph","content":[{"type":"text","text":"Done task"}]}]},
      {"type":"taskItem","attrs":{"status":"todo","checked":false},"content":[{"type":"paragraph","content":[{"type":"text","text":"Open task"}]}]}]},
    {"type":"blockquote","content":[{"type":"paragraph","content":[{"type":"text","text":"A quoted paragraph."}]}]},
    {"type":"codeBlock","content":[{"type":"text","text":"let note = open(session);\nnote.save();"}]},
    {"type":"horizontalRule"},
    {"type":"paragraph","content":[{"type":"text","text":"After the rule."}]}]}',
    0
  ),
  (
    's-critique',
    's-critique',
    'note',
    '',
    'prosemirror_json',
    '{"type":"doc","content":[{"type":"paragraph","content":[{"type":"text","text":"Bring the onboarding mocks."}]}]}',
    0
  ),
  (
    's-critique-summary',
    's-critique',
    'summary',
    '',
    'prosemirror_json',
    '{"type":"doc","content":[
    {"type":"heading","attrs":{"level":2},"content":[{"type":"text","text":"Decisions"}]},
    {"type":"bulletList","content":[
      {"type":"listItem","content":[{"type":"paragraph","content":[{"type":"text","text":"Ship the simplified onboarding in the next release."}]}]},
      {"type":"listItem","content":[{"type":"paragraph","content":[{"type":"text","text":"Keep the calendar step optional."}]}]}]},
    {"type":"heading","attrs":{"level":2},"content":[{"type":"text","text":"Follow-ups"}]},
    {"type":"paragraph","content":[{"type":"text","text":"Mina shares updated mocks by Friday."}]}]}',
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
    't-critique',
    's-critique',
    0,
    95000,
    '[
    {"id":"w1","text":"Let''s","start_ms":1000,"end_ms":1300,"channel":0},
    {"id":"w2","text":" start","start_ms":1300,"end_ms":1600,"channel":0},
    {"id":"w3","text":" with","start_ms":1600,"end_ms":1800,"channel":0},
    {"id":"w4","text":" onboarding.","start_ms":1800,"end_ms":2400,"channel":0},
    {"id":"w5","text":"The","start_ms":4000,"end_ms":4200,"channel":1},
    {"id":"w6","text":" calendar","start_ms":4200,"end_ms":4700,"channel":1},
    {"id":"w7","text":" step","start_ms":4700,"end_ms":5000,"channel":1},
    {"id":"w8","text":" should","start_ms":5000,"end_ms":5300,"channel":1},
    {"id":"w9","text":" be","start_ms":5300,"end_ms":5400,"channel":1},
    {"id":"w10","text":" optional.","start_ms":5400,"end_ms":6000,"channel":1},
    {"id":"w11","text":"Agreed,","start_ms":62000,"end_ms":62600,"channel":0},
    {"id":"w12","text":" let''s","start_ms":62600,"end_ms":62900,"channel":0},
    {"id":"w13","text":" ship","start_ms":62900,"end_ms":63300,"channel":0},
    {"id":"w14","text":" it.","start_ms":63300,"end_ms":63700,"channel":0}]'
  );
