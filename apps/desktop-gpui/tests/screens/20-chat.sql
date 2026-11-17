-- Chat history for the "Ask anything" screens. A configured (keyless) local model makes
-- both apps show the composer; nothing is sent while rendering.
INSERT INTO
  app_settings (id, value_json)
VALUES
  ('current_llm_provider', '"ollama"'),
  ('current_llm_model', '"llama3.2"');
INSERT INTO
  chat_groups (id, title, created_at, updated_at)
VALUES
  (
    'g-launch',
    'Launch plan follow-ups',
    strftime('%Y-%m-%dT%H:%M:%fZ', 'now', '-2 hours'),
    strftime('%Y-%m-%dT%H:%M:%fZ', 'now', '-2 hours')
  ),
  (
    'g-hiring',
    'Hiring debrief questions',
    strftime('%Y-%m-%dT%H:%M:%fZ', 'now', '-1 day'),
    strftime('%Y-%m-%dT%H:%M:%fZ', 'now', '-1 day')
  ),
  (
    'g-pricing',
    'Pricing readout summary',
    strftime('%Y-%m-%dT%H:%M:%fZ', 'now', '-6 days'),
    strftime('%Y-%m-%dT%H:%M:%fZ', 'now', '-6 days')
  ),
  (
    'g-automation',
    'Automation run (hidden from general history)',
    strftime('%Y-%m-%dT%H:%M:%fZ', 'now', '-30 minutes'),
    strftime('%Y-%m-%dT%H:%M:%fZ', 'now', '-30 minutes')
  );
INSERT INTO
  chat_messages (
    id,
    chat_group_id,
    role,
    content,
    metadata_json,
    parts_json,
    created_at
  )
VALUES
  (
    'm-launch-1',
    'g-launch',
    'user',
    'What did we decide in the weekly product sync?',
    '{}',
    '[{"type":"text","text":"What did we decide in the weekly product sync?"}]',
    strftime('%Y-%m-%dT%H:%M:%fZ', 'now', '-2 hours')
  ),
  (
    'm-launch-2',
    'g-launch',
    'assistant',
    '',
    '{}',
    '[{"type":"step-start"},{"type":"reasoning","text":"Looking up the weekly product sync notes.","state":"done"},{"type":"tool-search_meetings","toolCallId":"call-1","state":"output-available","input":{"query":"weekly product sync"},"output":[]},{"type":"text","state":"done","text":"Here is a recap of the **Weekly product sync**.\n\n### Decisions\n- Ship the beta to design partners on **Friday**.\n- Keep the `chat` flag on for internal builds only.\n\n### Action items\n1. Ana drafts the release notes.\n2. Ben confirms the QA matrix with [the release checklist](https://example.com/release).\n\n| Owner | Task | Due |\n|---|---|---|\n| Ana | Release notes | Thu |\n| Ben | QA matrix | Fri |"}]',
    strftime(
      '%Y-%m-%dT%H:%M:%fZ',
      'now',
      '-2 hours',
      '+20 seconds'
    )
  ),
  (
    'm-launch-3',
    'g-launch',
    'user',
    'Who owns the QA matrix?',
    '{}',
    '[{"type":"text","text":"Who owns the QA matrix?"}]',
    strftime(
      '%Y-%m-%dT%H:%M:%fZ',
      'now',
      '-2 hours',
      '+2 minutes'
    )
  ),
  (
    'm-launch-4',
    'g-launch',
    'assistant',
    '',
    '{}',
    '[{"type":"step-start"},{"type":"text","state":"done","text":"**Ben** owns the QA matrix and will confirm it by Friday."}]',
    strftime(
      '%Y-%m-%dT%H:%M:%fZ',
      'now',
      '-2 hours',
      '+2 minutes',
      '+10 seconds'
    )
  ),
  (
    'm-hiring-1',
    'g-hiring',
    'user',
    'Which questions came up in the hiring debrief?',
    '{}',
    '[{"type":"text","text":"Which questions came up in the hiring debrief?"}]',
    strftime('%Y-%m-%dT%H:%M:%fZ', 'now', '-1 day')
  ),
  (
    'm-hiring-2',
    'g-hiring',
    'assistant',
    '',
    '{}',
    '[{"type":"step-start"},{"type":"reasoning","text":"Reading the hiring debrief.","state":"done"},{"type":"tool-search_meetings","toolCallId":"call-2","state":"output-available","input":{"query":"hiring debrief"},"output":[]},{"type":"text","state":"done","text":"Two questions came up.\n\n> Can they own the release process end to end?\n\n- Ownership of **on-call** rotations\n- Pairing on the `sync` engine\n\n```ts\nconst panelWidth = 320;\n```"}]',
    strftime(
      '%Y-%m-%dT%H:%M:%fZ',
      'now',
      '-1 day',
      '+15 seconds'
    )
  ),
  (
    'm-automation-1',
    'g-automation',
    'user',
    'Summarize new notes',
    '{"chatScope":"automations"}',
    '[{"type":"text","text":"Summarize new notes"}]',
    strftime('%Y-%m-%dT%H:%M:%fZ', 'now', '-30 minutes')
  );
