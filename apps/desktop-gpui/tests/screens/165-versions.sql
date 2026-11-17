-- The offsite note was edited on two devices at once: an unresolved body conflict plus the
-- earlier versions its history lists (the conflict banner and version history dialog).
INSERT INTO
  session_documents (id, session_id, kind, body_format, body)
VALUES
  (
    's-offsite',
    's-offsite',
    'note',
    'prosemirror_json',
    '{"type":"doc","content":[
    {"type":"heading","attrs":{"level":2},"content":[{"type":"text","text":"Travel"}]},
    {"type":"bulletList","content":[
      {"type":"listItem","content":[{"type":"paragraph","content":[{"type":"text","text":"Book the train for Thursday morning"}]}]},
      {"type":"listItem","content":[{"type":"paragraph","content":[{"type":"text","text":"Hotel block holds 12 rooms until Friday"}]}]}]},
    {"type":"heading","attrs":{"level":2},"content":[{"type":"text","text":"Agenda"}]},
    {"type":"paragraph","content":[{"type":"text","text":"Day one is planning, day two is the customer panel."}]}]}'
  );
INSERT INTO
  e2ee_field_conflicts (
    id,
    table_name,
    row_id,
    field_name,
    lost_side,
    edited_at_ms,
    value_json,
    created_at
  )
VALUES
  (
    'conflict-offsite-body',
    'session_documents',
    's-offsite',
    'body',
    'local',
    CAST(
      (julianday('now', '-4 minutes') - 2440587.5) * 86400000 AS INTEGER
    ),
    json_quote(
      '{"type":"doc","content":[{"type":"heading","attrs":{"level":2},"content":[{"type":"text","text":"Travel"}]},{"type":"bulletList","content":[{"type":"listItem","content":[{"type":"paragraph","content":[{"type":"text","text":"Book the train for Wednesday evening"}]}]},{"type":"listItem","content":[{"type":"paragraph","content":[{"type":"text","text":"Hotel block holds 14 rooms until Friday"}]}]}]}]}'
    ),
    strftime('%Y-%m-%dT%H:%M:%fZ', 'now', '-3 minutes')
  );
INSERT INTO
  session_document_versions (
    id,
    document_id,
    session_id,
    body_format,
    body,
    source,
    created_at
  )
VALUES
  (
    'version-offsite-synced',
    's-offsite',
    's-offsite',
    'prosemirror_json',
    '{"type":"doc","content":[{"type":"heading","attrs":{"level":2},"content":[{"type":"text","text":"Travel"}]},{"type":"bulletList","content":[{"type":"listItem","content":[{"type":"paragraph","content":[{"type":"text","text":"Book the train for Thursday morning"}]}]}]},{"type":"paragraph","content":[{"type":"text","text":"Ask finance whether the venue deposit can move to next quarter, and confirm the dietary list with every attendee before the caterer deadline on Monday."}]}]}',
    'sync',
    strftime('%Y-%m-%dT%H:%M:%fZ', 'now', '-2 hours')
  ),
  (
    'version-offsite-local',
    's-offsite',
    's-offsite',
    'markdown',
    '## Travel

- Book the train',
    'local',
    strftime('%Y-%m-%dT%H:%M:%fZ', 'now', '-1 days')
  ),
  (
    'version-offsite-empty',
    's-offsite',
    's-offsite',
    'prosemirror_json',
    '{"type":"doc","content":[{"type":"paragraph"}]}',
    'local',
    strftime('%Y-%m-%dT%H:%M:%fZ', 'now', '-6 days')
  );
